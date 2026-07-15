use std::{
    collections::HashMap,
    fmt,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub const DEFAULT_ACTION_CAPACITY: usize = 256;
pub const DEFAULT_EFFECT_CAPACITY: usize = 64;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EffectId(u64);

impl EffectId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReductionContext {
    pub now_ms: u64,
}

pub trait Clock: Send + Sync {
    fn now_ms(&self) -> u64;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX)
    }
}

pub trait IdGenerator: Send + Sync {
    fn next_effect_id(&self) -> EffectId;
}

#[derive(Debug)]
pub struct AtomicIdGenerator {
    next: AtomicU64,
}

impl AtomicIdGenerator {
    #[must_use]
    pub const fn new(first: u64) -> Self {
        Self {
            next: AtomicU64::new(first),
        }
    }
}

impl Default for AtomicIdGenerator {
    fn default() -> Self {
        Self::new(1)
    }
}

impl IdGenerator for AtomicIdGenerator {
    fn next_effect_id(&self) -> EffectId {
        EffectId::new(self.next.fetch_add(1, Ordering::Relaxed))
    }
}

pub trait Reducer<S, A, E>: Send + Sync {
    fn reduce(&self, state: &S, action: &A, context: ReductionContext) -> Reduction<S, E>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reduction<S, E> {
    pub state: S,
    pub effects: Vec<E>,
}

impl<S, E> Reduction<S, E> {
    #[must_use]
    pub const fn new(state: S, effects: Vec<E>) -> Self {
        Self { state, effects }
    }
}

#[derive(Clone, Debug)]
pub struct EffectRequest<E> {
    pub id: EffectId,
    pub requested_at_ms: u64,
    pub effect: E,
    pub cancellation: CancellationToken,
}

#[derive(Clone, Debug)]
pub struct ActionSender<A> {
    inner: mpsc::Sender<A>,
}

impl<A> ActionSender<A> {
    pub async fn dispatch(&self, action: A) -> Result<(), DispatchError> {
        self.inner
            .send(action)
            .await
            .map_err(|_| DispatchError::RuntimeStopped)
    }

    pub fn try_dispatch(&self, action: A) -> Result<(), DispatchError> {
        self.inner.try_send(action).map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => DispatchError::Backpressure,
            mpsc::error::TrySendError::Closed(_) => DispatchError::RuntimeStopped,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DispatchError {
    #[error("the runtime action queue is full")]
    Backpressure,
    #[error("the runtime has stopped")]
    RuntimeStopped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuntimeError {
    #[error("{channel} channel capacity must be greater than zero")]
    InvalidCapacity { channel: &'static str },
    #[error("the effect worker queue closed unexpectedly")]
    EffectChannelClosed,
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeConfig<A> {
    pub action_capacity: usize,
    pub effect_capacity: usize,
    pub completed_effect: fn(&A) -> Option<EffectId>,
    pub cancellation_target: fn(&A) -> Option<EffectId>,
}

pub struct Runtime<S, A, E, R, C = SystemClock, I = AtomicIdGenerator> {
    state: S,
    reducer: R,
    clock: C,
    ids: I,
    actions: mpsc::Receiver<A>,
    effects: mpsc::Sender<EffectRequest<E>>,
    cancellations: HashMap<EffectId, CancellationToken>,
    completed_effect: fn(&A) -> Option<EffectId>,
    cancellation_target: fn(&A) -> Option<EffectId>,
}

pub type RuntimeParts<S, A, E, R, C = SystemClock, I = AtomicIdGenerator> = (
    Runtime<S, A, E, R, C, I>,
    ActionSender<A>,
    mpsc::Receiver<EffectRequest<E>>,
);

impl<S: fmt::Debug, A, E, R, C, I> fmt::Debug for Runtime<S, A, E, R, C, I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Runtime")
            .field("state", &self.state)
            .field("active_effects", &self.cancellations.len())
            .finish_non_exhaustive()
    }
}

impl<S, A, E, R, C, I> Runtime<S, A, E, R, C, I>
where
    R: Reducer<S, A, E>,
    C: Clock,
    I: IdGenerator,
{
    pub fn new(
        initial_state: S,
        reducer: R,
        clock: C,
        ids: I,
        config: RuntimeConfig<A>,
    ) -> Result<RuntimeParts<S, A, E, R, C, I>, RuntimeError> {
        if config.action_capacity == 0 {
            return Err(RuntimeError::InvalidCapacity { channel: "action" });
        }
        if config.effect_capacity == 0 {
            return Err(RuntimeError::InvalidCapacity { channel: "effect" });
        }

        let (action_sender, actions) = mpsc::channel(config.action_capacity);
        let (effects, effect_receiver) = mpsc::channel(config.effect_capacity);
        Ok((
            Self {
                state: initial_state,
                reducer,
                clock,
                ids,
                actions,
                effects,
                cancellations: HashMap::new(),
                completed_effect: config.completed_effect,
                cancellation_target: config.cancellation_target,
            },
            ActionSender {
                inner: action_sender,
            },
            effect_receiver,
        ))
    }

    #[must_use]
    pub const fn state(&self) -> &S {
        &self.state
    }

    #[must_use]
    pub fn active_effect_count(&self) -> usize {
        self.cancellations.len()
    }

    pub async fn step(&mut self, action: A) -> Result<(), RuntimeError> {
        if let Some(effect_id) = (self.cancellation_target)(&action)
            && let Some(token) = self.cancellations.get(&effect_id)
        {
            token.cancel();
        }

        if let Some(effect_id) = (self.completed_effect)(&action) {
            self.cancellations.remove(&effect_id);
        }

        let requested_at_ms = self.clock.now_ms();
        let reduction = self.reducer.reduce(
            &self.state,
            &action,
            ReductionContext {
                now_ms: requested_at_ms,
            },
        );
        self.state = reduction.state;

        for effect in reduction.effects {
            let id = self.ids.next_effect_id();
            let cancellation = CancellationToken::new();
            self.cancellations.insert(id, cancellation.clone());
            if self
                .effects
                .send(EffectRequest {
                    id,
                    requested_at_ms,
                    effect,
                    cancellation,
                })
                .await
                .is_err()
            {
                self.cancellations.remove(&id);
                return Err(RuntimeError::EffectChannelClosed);
            }
        }
        Ok(())
    }

    pub async fn run(mut self) -> Result<S, RuntimeError> {
        while let Some(action) = self.actions.recv().await {
            self.step(action).await?;
        }
        Ok(self.state)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Clone, Copy, Debug)]
    struct FixedClock(u64);

    impl Clock for FixedClock {
        fn now_ms(&self) -> u64 {
            self.0
        }
    }

    #[derive(Debug)]
    struct SequentialIds(Mutex<u64>);

    impl IdGenerator for SequentialIds {
        fn next_effect_id(&self) -> EffectId {
            let mut next = self.0.lock().unwrap_or_else(|error| error.into_inner());
            let id = *next;
            *next = next.saturating_add(1);
            EffectId::new(id)
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum TestAction {
        Increment,
        Finish(EffectId),
        Cancel(EffectId),
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct TestEffect(u64);

    #[derive(Clone, Copy, Debug)]
    struct TestReducer;

    impl Reducer<u64, TestAction, TestEffect> for TestReducer {
        fn reduce(
            &self,
            state: &u64,
            action: &TestAction,
            _context: ReductionContext,
        ) -> Reduction<u64, TestEffect> {
            match action {
                TestAction::Increment => Reduction::new(state + 1, vec![TestEffect(state + 1)]),
                TestAction::Finish(_) | TestAction::Cancel(_) => Reduction::new(*state, vec![]),
            }
        }
    }

    fn completed(action: &TestAction) -> Option<EffectId> {
        match action {
            TestAction::Finish(id) => Some(*id),
            TestAction::Increment | TestAction::Cancel(_) => None,
        }
    }

    fn cancelled(action: &TestAction) -> Option<EffectId> {
        match action {
            TestAction::Cancel(id) => Some(*id),
            TestAction::Increment | TestAction::Finish(_) => None,
        }
    }

    type TestRuntime = Runtime<u64, TestAction, TestEffect, TestReducer, FixedClock, SequentialIds>;
    type TestRuntimeBundle = (
        TestRuntime,
        ActionSender<TestAction>,
        mpsc::Receiver<EffectRequest<TestEffect>>,
    );

    fn runtime(
        action_capacity: usize,
        effect_capacity: usize,
    ) -> Result<TestRuntimeBundle, RuntimeError> {
        Runtime::new(
            0,
            TestReducer,
            FixedClock(1234),
            SequentialIds(Mutex::new(7)),
            RuntimeConfig {
                action_capacity,
                effect_capacity,
                completed_effect: completed,
                cancellation_target: cancelled,
            },
        )
    }

    #[tokio::test]
    async fn assigns_deterministic_effect_metadata() -> Result<(), Box<dyn std::error::Error>> {
        let (mut runtime, _sender, mut effects) = runtime(2, 2)?;
        runtime.step(TestAction::Increment).await?;

        let request = effects.recv().await.ok_or("missing effect")?;
        assert_eq!(request.id, EffectId::new(7));
        assert_eq!(request.requested_at_ms, 1234);
        assert_eq!(request.effect, TestEffect(1));
        assert!(!request.cancellation.is_cancelled());
        assert_eq!(runtime.state(), &1);
        assert_eq!(runtime.active_effect_count(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn cancellation_and_completion_manage_tokens() -> Result<(), Box<dyn std::error::Error>> {
        let (mut runtime, _sender, mut effects) = runtime(2, 2)?;
        runtime.step(TestAction::Increment).await?;
        let request = effects.recv().await.ok_or("missing effect")?;

        runtime.step(TestAction::Cancel(request.id)).await?;
        assert!(request.cancellation.is_cancelled());
        assert_eq!(runtime.active_effect_count(), 1);

        runtime.step(TestAction::Finish(request.id)).await?;
        assert_eq!(runtime.active_effect_count(), 0);
        Ok(())
    }

    #[test]
    fn bounded_action_channel_reports_backpressure() -> Result<(), Box<dyn std::error::Error>> {
        let (_runtime, sender, _effects) = runtime(1, 1)?;

        sender.try_dispatch(TestAction::Increment)?;
        assert_eq!(
            sender.try_dispatch(TestAction::Increment),
            Err(DispatchError::Backpressure)
        );
        Ok(())
    }

    #[test]
    fn rejects_zero_capacity() {
        assert!(matches!(
            runtime(0, 1),
            Err(RuntimeError::InvalidCapacity { channel: "action" })
        ));
        assert!(matches!(
            runtime(1, 0),
            Err(RuntimeError::InvalidCapacity { channel: "effect" })
        ));
    }

    #[test]
    fn replay_produces_identical_state_and_effects() {
        let actions = [TestAction::Increment, TestAction::Increment];
        let replay = || {
            actions
                .iter()
                .fold((0, Vec::new()), |(state, mut effects), action| {
                    let reduction =
                        TestReducer.reduce(&state, action, ReductionContext { now_ms: 55 });
                    effects.extend(reduction.effects);
                    (reduction.state, effects)
                })
        };

        assert_eq!(replay(), replay());
        assert_eq!(replay(), (2, vec![TestEffect(1), TestEffect(2)]));
    }
}

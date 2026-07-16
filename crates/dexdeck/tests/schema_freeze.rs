use dexdeck_config::SESSION_SCHEMA_VERSION;
use dexdeck_core::TRUST_SCHEMA_VERSION;
use dexdeck_protocol::{
    BRIDGE_PROTOCOL_VERSION, CACHE_SCHEMA_VERSION, CLI_SCHEMA_VERSION, CONFIG_SCHEMA_VERSION,
    JOB_HISTORY_SCHEMA_VERSION, LOG_FILTERS_SCHEMA_VERSION,
};

#[test]
fn v0_2_0_contracts_are_frozen_at_schema_v1() {
    assert_eq!(
        [
            CLI_SCHEMA_VERSION,
            BRIDGE_PROTOCOL_VERSION,
            CONFIG_SCHEMA_VERSION,
            CACHE_SCHEMA_VERSION,
            JOB_HISTORY_SCHEMA_VERSION,
            LOG_FILTERS_SCHEMA_VERSION,
            SESSION_SCHEMA_VERSION,
            TRUST_SCHEMA_VERSION,
        ],
        [1; 8]
    );
}

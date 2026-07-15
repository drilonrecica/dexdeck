//! Canonical public identity values.

pub const PRODUCT_NAME: &str = "DexDeck";
pub const EXECUTABLE_NAME: &str = "dexdeck";
pub const ENVIRONMENT_PREFIX: &str = "DEXDECK_";
pub const PROJECT_DIRECTORY: &str = ".dexdeck";
pub const CACHE_NAMESPACE: &str = "dexdeck";
pub const REPOSITORY: &str = "drilonrecica/dexdeck";
pub const DEFAULT_THEME: &str = "lazuli";
pub const LOGO_SYMBOL: &str = "Deckmark";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_identifiers_are_stable() {
        assert_eq!(PRODUCT_NAME, "DexDeck");
        assert_eq!(EXECUTABLE_NAME, "dexdeck");
        assert_eq!(ENVIRONMENT_PREFIX, "DEXDECK_");
        assert_eq!(PROJECT_DIRECTORY, ".dexdeck");
        assert_eq!(CACHE_NAMESPACE, "dexdeck");
        assert_eq!(REPOSITORY, "drilonrecica/dexdeck");
        assert_eq!(DEFAULT_THEME, "lazuli");
        assert_eq!(LOGO_SYMBOL, "Deckmark");
    }
}

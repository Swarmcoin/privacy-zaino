//! Network type for Zaino configuration.

use std::fmt;

use serde::{Deserialize, Serialize};
use zebra_chain::parameters::testnet::ConfiguredActivationHeights;

/// The network kind Zaino is configured for. Activation heights are chain
/// facts the validator owns; the backends adopt the runtime schedule from the
/// validator's `getblockchaininfo.upgrades` at spawn and hold it as a
/// `zebra_chain::parameters::Network`
/// (<https://github.com/zingolabs/zaino/issues/1076>). A pre-adoption
/// height read is unrepresentable. A custom testnet additionally pins the
/// expected genesis and schedule, which must agree with the validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Network {
    /// Mainnet network
    Mainnet,
    /// The Public Testnet: the shared-consensus test network, whose
    /// activation heights come only from checked-in zebra parameters —
    /// never from configuration. Accepts the legacy config spelling
    /// `"Testnet"`.
    #[serde(alias = "Testnet")]
    PubTestnet,
    /// Regtest network (for local testing)
    Regtest,
    /// An isolated PoW testnet using standard testnet address encodings.
    /// These expected chain facts are checked before the index is opened.
    CustomTestnet {
        /// Expected height-zero block hash, not the public testnet genesis.
        #[serde(
            serialize_with = "serialize_genesis_hash",
            deserialize_with = "deserialize_genesis_hash"
        )]
        genesis_hash: zebra_chain::block::Hash,
        /// Exact expected upgrade schedule; absent upgrades stay disabled.
        activation_heights: ActivationHeights,
    },
    /// The SWARM production network: a separate chain from Zcash Mainnet, with
    /// its own address encodings
    /// ([`zcash_protocol::consensus::NetworkType::SwarmMain`]), its own
    /// consensus branch domain and its own genesis.
    ///
    /// [`Network::Mainnet`] above is Zcash Mainnet and stays that way, so an
    /// operator cannot reach this chain by writing `Mainnet`.
    ///
    /// Both fields are required. There is no default genesis to fall back to:
    /// the production genesis is generated at the launch ceremony, and an
    /// indexer that opened a store against the wrong chain would index it as
    /// this one.
    SwarmMain {
        /// Expected height-zero block hash, checked against the validator
        /// before the index is opened.
        #[serde(
            serialize_with = "serialize_genesis_hash",
            deserialize_with = "deserialize_genesis_hash"
        )]
        genesis_hash: zebra_chain::block::Hash,
        /// Exact expected upgrade schedule; absent upgrades stay disabled.
        activation_heights: ActivationHeights,
    },
}

fn serialize_genesis_hash<S: serde::Serializer>(
    hash: &zebra_chain::block::Hash,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_str(hash)
}

fn deserialize_genesis_hash<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<zebra_chain::block::Hash, D::Error> {
    String::deserialize(deserializer)?
        .parse()
        .map_err(serde::de::Error::custom)
}

/// The display name of the project's custom testnet, also used as the
/// `network_name` of the zebra `Parameters` built for it. Alphanumeric and
/// within zebra's network-name bound, as `with_network_name` requires.
pub const CUSTOM_TESTNET_DISPLAY_NAME: &str = "SwarmTestnet";

/// The chain label light wallets receive for the project's custom testnet in
/// `GetLightdInfo.chain_name`. Clients pin this string, so it is a public
/// interface: changing it re-identifies the chain to every wallet.
pub const CUSTOM_TESTNET_CHAIN_NAME: &str = "swarm-testnet";

/// The display name of the SWARM production network, also used as the
/// `network_name` of the zebra `Parameters` built for it. Alphanumeric and
/// within zebra's network-name bound, as `with_network_name` requires.
pub const SWARM_MAINNET_DISPLAY_NAME: &str = "SwarmMainnet";

/// The chain label light wallets receive for the SWARM production network in
/// `GetLightdInfo.chain_name`. Clients pin this string, so it is a public
/// interface: changing it re-identifies the chain to every wallet. It is not
/// `main`, which is Zcash Mainnet's label and stays Zcash Mainnet's.
pub const SWARM_MAINNET_CHAIN_NAME: &str = "swarm-mainnet";

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Network::Mainnet => write!(f, "Mainnet"),
            Network::PubTestnet => write!(f, "PubTestnet"),
            Network::Regtest => write!(f, "Regtest"),
            Network::CustomTestnet { .. } => write!(f, "{CUSTOM_TESTNET_DISPLAY_NAME}"),
            Network::SwarmMain { .. } => write!(f, "{SWARM_MAINNET_DISPLAY_NAME}"),
        }
    }
}

/// Configurable activation heights for a Regtest validator launch.
///
/// We use our own type instead of the zebra type
/// as the zebra type is missing a number of useful
/// traits, notably Debug, PartialEq, and Eq
///
/// This also allows us to define our own set
/// of defaults
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Copy)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct ActivationHeights {
    /// Activation height for `BeforeOverwinter` network upgrade.
    pub before_overwinter: Option<u32>,
    /// Activation height for `Overwinter` network upgrade.
    pub overwinter: Option<u32>,
    /// Activation height for `Sapling` network upgrade.
    pub sapling: Option<u32>,
    /// Activation height for `Blossom` network upgrade.
    pub blossom: Option<u32>,
    /// Activation height for `Heartwood` network upgrade.
    pub heartwood: Option<u32>,
    /// Activation height for `Canopy` network upgrade.
    pub canopy: Option<u32>,
    /// Activation height for `NU5` network upgrade.
    #[serde(rename = "NU5")]
    pub nu5: Option<u32>,
    /// Activation height for `NU6` network upgrade.
    #[serde(rename = "NU6")]
    pub nu6: Option<u32>,
    /// Activation height for `NU6.1` network upgrade.
    /// see <https://zips.z.cash/#nu6-1-candidate-zips> for info on NU6.1
    #[serde(rename = "NU6.1")]
    pub nu6_1: Option<u32>,
    /// Activation height for `NU6.2` network upgrade.
    #[serde(rename = "NU6.2")]
    pub nu6_2: Option<u32>,
    /// Activation height for `NU6.3` network upgrade.
    #[serde(rename = "NU6.3")]
    pub nu6_3: Option<u32>,
    /// Activation height for `NU7` network upgrade.
    #[serde(rename = "NU7")]
    pub nu7: Option<u32>,
}

impl Default for ActivationHeights {
    fn default() -> Self {
        ActivationHeights {
            before_overwinter: Some(1),
            overwinter: Some(1),
            sapling: Some(1),
            blossom: Some(1),
            heartwood: Some(1),
            canopy: Some(1),
            nu5: Some(2),
            nu6: Some(2),
            nu6_1: Some(2),
            nu6_2: Some(2),
            nu6_3: None,
            nu7: None,
        }
    }
}

/// Records the `NetworkUpgrade`-variant ↔ `ActivationHeights`-field correspondence
/// exactly once, generating everything derived from it: the two field-by-field
/// `From` conversions between [`ActivationHeights`] and zebra's
/// [`ConfiguredActivationHeights`] (the structs share field names), the
/// all-`None` [`ActivationHeights::NEVER_ACTIVATED`] schedule, and the
/// per-upgrade [`ActivationHeights::slot_mut`] accessor.
///
/// A declarative macro rather than functions because plain `fn`s cannot abstract
/// over struct fields, and the variant/field spellings (`Nu5`/`nu5`) differ only
/// by casing, which `macro_rules!` cannot derive — hence explicit pairs.
///
/// Zebra's side of these conversions is structurally stable; the recurring edit
/// here is a new network upgrade, which lands as a single `(Variant, field)`
/// entry in the invocation below (after adding the struct field). The exhaustive
/// destructures and match keep full compile-time drift detection: a new zebra
/// field or variant fails the build until its pair is added.
macro_rules! activation_heights_mirror {
    ($(($variant:ident, $field:ident)),* $(,)?) => {
        impl From<ConfiguredActivationHeights> for ActivationHeights {
            fn from(
                ConfiguredActivationHeights { $($field),* }: ConfiguredActivationHeights,
            ) -> Self {
                Self { $($field),* }
            }
        }

        impl From<ActivationHeights> for ConfiguredActivationHeights {
            fn from(ActivationHeights { $($field),* }: ActivationHeights) -> Self {
                Self { $($field),* }
            }
        }

        impl ActivationHeights {
            /// The all-`None` schedule: every upgrade never-activated. The
            /// starting point for building heights from an external report,
            /// where an upgrade absent from the report must stay unset.
            pub const NEVER_ACTIVATED: Self = Self { $($field: None),* };

            /// Mutable slot for `upgrade`'s activation height, or `None` for
            /// `Genesis` (height 0 by definition; it has no slot).
            pub fn slot_mut(
                &mut self,
                upgrade: zebra_chain::parameters::NetworkUpgrade,
            ) -> Option<&mut Option<u32>> {
                match upgrade {
                    // Genesis is height 0 by definition. SwarmMain is the SWARM
                    // production transaction domain (`SWM1`), which the node never
                    // reports as a schedule entry; neither has a slot.
                    zebra_chain::parameters::NetworkUpgrade::Genesis
                    | zebra_chain::parameters::NetworkUpgrade::SwarmMain => None,
                    $(
                        zebra_chain::parameters::NetworkUpgrade::$variant => {
                            Some(&mut self.$field)
                        }
                    )*
                }
            }
        }
    };
}

activation_heights_mirror!(
    (BeforeOverwinter, before_overwinter),
    (Overwinter, overwinter),
    (Sapling, sapling),
    (Blossom, blossom),
    (Heartwood, heartwood),
    (Canopy, canopy),
    (Nu5, nu5),
    (Nu6, nu6),
    (Nu6_1, nu6_1),
    (Nu6_2, nu6_2),
    (Nu6_3, nu6_3),
    (Nu7, nu7),
);

impl Network {
    /// Determines if we should wait for the server to fully sync. Used for testing
    ///
    /// - Mainnet / The Public Testnet: Skip sync (false) because we don't want
    ///   to sync real chains in tests
    /// - Regtest: Enable sync (true) because regtest is local and fast to sync
    pub fn wait_on_server_sync(&self) -> bool {
        match self {
            // Real networks - don't try to sync the whole chain
            Network::Mainnet | Network::PubTestnet | Network::SwarmMain { .. } => false,
            // Local network - safe and fast to sync
            Network::Regtest | Network::CustomTestnet { .. } => true,
        }
    }

    /// The `chain_name` this network reports to light wallets in
    /// `GetLightdInfo`.
    ///
    /// Not the validator's own RPC label: zebra answers `test` for both the
    /// public testnet and regtest, and answers `test` for a configured testnet
    /// too, so a client reading the validator's string cannot tell which chain
    /// it is talking to. The configured kind is the only thing that can, and
    /// clients select address prefixes and the upgrade schedule from it, so
    /// each kind owns one stable string here.
    pub fn lightwallet_chain_name(&self) -> &'static str {
        match self {
            Network::Mainnet => "main",
            Network::PubTestnet => "test",
            Network::Regtest => "regtest",
            Network::CustomTestnet { .. } => CUSTOM_TESTNET_CHAIN_NAME,
            Network::SwarmMain { .. } => SWARM_MAINNET_CHAIN_NAME,
        }
    }

    /// The network type this kind's addresses are encoded for.
    ///
    /// Not derived from the runtime `zebra_chain::parameters::Network`: zebra
    /// has two kinds, `Mainnet` and `Testnet`, and answers `Test` for every
    /// configured testnet, so a SWARM production indexer reading its network
    /// type from zebra would accept SwarmTestnet addresses and refuse its own.
    /// The configured kind is the only thing that knows.
    pub fn network_type(&self) -> zcash_protocol::consensus::NetworkType {
        use zcash_protocol::consensus::NetworkType;
        match self {
            Network::Mainnet => NetworkType::Main,
            // SwarmTestnet uses the standard testnet encodings, with the
            // project's own unified HRPs inside them.
            Network::PubTestnet | Network::CustomTestnet { .. } => NetworkType::Test,
            Network::Regtest => NetworkType::Regtest,
            Network::SwarmMain { .. } => NetworkType::SwarmMain,
        }
    }

    /// The genesis hash this kind pins, for the kinds that pin one.
    ///
    /// `None` for the chains whose genesis is a compiled zebra parameter or,
    /// on regtest, whatever the local validator made.
    pub fn expected_genesis_hash(&self) -> Option<zebra_chain::block::Hash> {
        match self {
            Network::Mainnet | Network::PubTestnet | Network::Regtest => None,
            Network::CustomTestnet { genesis_hash, .. }
            | Network::SwarmMain { genesis_hash, .. } => Some(*genesis_hash),
        }
    }

    /// The genesis hash this kind reports to light wallets in
    /// `GetLightdInfo.genesis_hash`, as 64 lowercase hexadecimal characters in
    /// display order.
    ///
    /// Not the same question as [`Network::expected_genesis_hash`], which is
    /// "what did the operator pin for this store to be checked against". This
    /// one is "what chain is this server serving", and the two upstream chains
    /// answer it from compiled zebra parameters rather than from configuration.
    ///
    /// `chain_name` alone cannot answer it: two chains built from the same
    /// software report the same label, so a wallet that trusted the label would
    /// sync against a rehearsal chain and write its state back. The genesis is
    /// the chain's identity.
    ///
    /// `None` only for regtest, whose genesis is whatever the local validator
    /// made and which this kind therefore cannot state without asking it. The
    /// caller sends the empty string for `None`, which is what an older server
    /// sends too, so a client must read `""` as "this server did not say".
    pub fn reported_genesis_hash(&self) -> Option<zebra_chain::block::Hash> {
        match self {
            // Zcash Mainnet and the public testnet are fixed chains. Their
            // hashes are read out of zebra rather than written again here, so
            // there is one copy of each in the build and no chance of a
            // transposed character making this server claim to be a chain it is
            // not.
            Network::Mainnet => Some(zebra_chain::parameters::Network::Mainnet.genesis_hash()),
            Network::PubTestnet => {
                Some(zebra_chain::parameters::Network::new_default_testnet().genesis_hash())
            }
            Network::Regtest => None,
            // The two SWARM chains carry theirs in the configuration, already
            // checked against the validator before the index was opened, so
            // reporting it cannot disagree with what the store holds.
            Network::CustomTestnet { genesis_hash, .. }
            | Network::SwarmMain { genesis_hash, .. } => Some(*genesis_hash),
        }
    }

    /// [`Network::reported_genesis_hash`] spelled for the wire: 64 lowercase
    /// hexadecimal characters in display order, or the empty string for a kind
    /// that pins no genesis.
    ///
    /// This is the whole of what `GetLightdInfo.genesis_hash` carries, kept here
    /// rather than at the call site so that the exact string a wallet compares
    /// against is the string the tests in this module assert.
    pub fn reported_genesis_hex(&self) -> String {
        self.reported_genesis_hash()
            .map(|hash| hash.to_string())
            .unwrap_or_default()
    }
}

impl From<zebra_chain::parameters::Network> for Network {
    fn from(value: zebra_chain::parameters::Network) -> Self {
        match value {
            zebra_chain::parameters::Network::Mainnet => Network::Mainnet,
            zebra_chain::parameters::Network::Testnet(parameters) => {
                if parameters.is_regtest() {
                    Network::Regtest
                } else {
                    Network::PubTestnet
                }
            }
        }
    }
}

impl ActivationHeights {
    /// Builds the runtime regtest network for a chain whose schedule is
    /// known first-hand: a validator being *launched* with these heights, or
    /// a test fixture that is its own chain (mockchain sources, proptest
    /// block generators). Production indexer code never calls this with
    /// configured values — it adopts the runtime network from the
    /// validator's reported schedule instead (zaino#1076).
    pub fn to_regtest_network(&self) -> zebra_chain::parameters::Network {
        zebra_chain::parameters::Network::new_regtest(
            Into::<ConfiguredActivationHeights>::into(*self).into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActivationHeights, Network, CUSTOM_TESTNET_CHAIN_NAME, CUSTOM_TESTNET_DISPLAY_NAME,
    };

    /// The project's custom testnet, with a genesis that is deliberately not
    /// the public testnet's. Only the discriminant matters to the label.
    fn custom_testnet() -> Network {
        Network::CustomTestnet {
            genesis_hash: "01d6e85dd3c1c128941a849c5025cd2e437258811a2551b82aefd68686c982e1"
                .parse()
                .expect("a 32-byte hex block hash"),
            activation_heights: ActivationHeights {
                nu6_3: Some(1),
                nu7: None,
                ..ActivationHeights::default()
            },
        }
    }

    /// The string light wallets pin for this chain. A wallet compiled for
    /// `swarm-testnet` refuses any other label, so this is the network's
    /// public identity, not a cosmetic name.
    #[test]
    fn custom_testnet_reports_the_swarm_chain_name() {
        assert_eq!(custom_testnet().lightwallet_chain_name(), "swarm-testnet");
        assert_eq!(CUSTOM_TESTNET_CHAIN_NAME, "swarm-testnet");
    }

    /// The upstream kinds keep the labels lightwalletd has always sent, so
    /// renaming the custom testnet cannot have moved one of them.
    #[test]
    fn upstream_networks_keep_their_lightwallet_chain_names() {
        assert_eq!(Network::Mainnet.lightwallet_chain_name(), "main");
        assert_eq!(Network::PubTestnet.lightwallet_chain_name(), "test");
        assert_eq!(Network::Regtest.lightwallet_chain_name(), "regtest");
    }

    /// The bug this mapping exists to prevent: zebra answers `test` for the
    /// public testnet, for regtest and for a configured testnet alike, so two
    /// kinds sharing a label would leave a wallet unable to tell which chain
    /// it reached.
    #[test]
    fn every_network_kind_has_a_distinct_lightwallet_chain_name() {
        let labels = [
            Network::Mainnet.lightwallet_chain_name(),
            Network::PubTestnet.lightwallet_chain_name(),
            Network::Regtest.lightwallet_chain_name(),
            custom_testnet().lightwallet_chain_name(),
        ];
        let mut sorted = labels;
        sorted.sort_unstable();
        assert!(
            sorted.windows(2).all(|pair| pair[0] != pair[1]),
            "two network kinds report the same label: {labels:?}"
        );
    }

    /// The SWARM production network, with a genesis standing in for the one the
    /// launch ceremony will produce. There is no constant for it, by design.
    fn swarm_mainnet() -> Network {
        Network::SwarmMain {
            genesis_hash: CEREMONY_GENESIS.parse().expect("a 32-byte hex block hash"),
            activation_heights: ActivationHeights {
                nu6_3: Some(1),
                nu7: None,
                ..ActivationHeights::default()
            },
        }
    }

    /// A genesis nobody has yet. Written out rather than derived so a reader can
    /// see that the profile is built from a value and not from a constant.
    const CEREMONY_GENESIS: &str =
        "00d4b1cb01d6bd2d1a3a4a49bba6fd0a4c2e2f7c0d6e5b4a39281706f5e4d3c2";

    /// The genesis of SwarmTestnet, as `custom_testnet()` above configures it.
    const CUSTOM_TESTNET_GENESIS: &str =
        "01d6e85dd3c1c128941a849c5025cd2e437258811a2551b82aefd68686c982e1";

    /// Each SWARM chain reports the genesis it was configured with, not a
    /// compiled-in one. This is the field a wallet compares before syncing, so
    /// reporting a different chain's hash here is reporting a different chain.
    #[test]
    fn the_swarm_chains_report_the_genesis_they_were_configured_with() {
        assert_eq!(
            custom_testnet().reported_genesis_hex(),
            CUSTOM_TESTNET_GENESIS,
        );
        assert_eq!(swarm_mainnet().reported_genesis_hex(), CEREMONY_GENESIS);
        // A second ceremony is a second chain, and the report follows it rather
        // than any value fixed at build time.
        let other = Network::SwarmMain {
            genesis_hash: "ab".repeat(32).parse().expect("a 32-byte hex block hash"),
            activation_heights: ActivationHeights::default(),
        };
        assert_eq!(other.reported_genesis_hex(), "ab".repeat(32));
        assert_ne!(
            other.reported_genesis_hex(),
            swarm_mainnet().reported_genesis_hex()
        );
    }

    /// The two fixed upstream chains report their published genesis, read out of
    /// zebra rather than written again here. The literals below are what
    /// `zcash-cli getblockhash 0` prints on each chain.
    #[test]
    fn the_upstream_chains_report_their_published_genesis() {
        assert_eq!(
            Network::Mainnet.reported_genesis_hex(),
            "00040fe8ec8471911baa1db1266ea15dd06b4a8a5c453883c000b031973dce08",
        );
        assert_eq!(
            Network::PubTestnet.reported_genesis_hex(),
            "05a60a92d99d85997cce3b87616c089f6124d7342af37106edc76126334a2c38",
        );
    }

    /// Regtest's genesis is whatever the local validator made, so this kind
    /// cannot state it and says nothing instead. The empty string is also what
    /// an older server sends, and a client must read it as "not stated" rather
    /// than as a hash that failed to match.
    #[test]
    fn regtest_states_no_genesis() {
        assert_eq!(Network::Regtest.reported_genesis_hash(), None);
        assert_eq!(Network::Regtest.reported_genesis_hex(), "");
    }

    /// Every stated genesis is the 64 lowercase hexadecimal characters of a
    /// block hash in display order — the spelling a node prints and a wallet
    /// holds. An upper-case or byte-reversed rendering would fail every
    /// comparison a client makes, while looking right in a log.
    #[test]
    fn every_stated_genesis_is_display_order_lowercase_hex() {
        for network in [
            Network::Mainnet,
            Network::PubTestnet,
            custom_testnet(),
            swarm_mainnet(),
        ] {
            let stated = network.reported_genesis_hex();
            assert_eq!(stated.len(), 64, "{network} stated {stated}");
            assert!(
                stated
                    .chars()
                    .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
                "{network} stated {stated}",
            );
            // Display order, not internal byte order: parsing the string back
            // must give the hash the kind reports.
            assert_eq!(
                stated.parse::<zebra_chain::block::Hash>().ok(),
                network.reported_genesis_hash(),
            );
        }
    }

    /// The point of the field: no two chains this build can serve state the same
    /// genesis, so a wallet that compares it can always tell them apart — which
    /// `chain_name` alone cannot do once two chains are built from the same
    /// software.
    #[test]
    fn no_two_chains_state_the_same_genesis() {
        let stated = [
            Network::Mainnet.reported_genesis_hex(),
            Network::PubTestnet.reported_genesis_hex(),
            custom_testnet().reported_genesis_hex(),
            swarm_mainnet().reported_genesis_hex(),
        ];
        let mut sorted = stated.clone();
        sorted.sort_unstable();
        assert!(
            sorted.windows(2).all(|pair| pair[0] != pair[1]),
            "two chains state the same genesis: {stated:?}",
        );
    }

    /// `expected_genesis_hash` answers "what did the operator pin for this store
    /// to be checked against" and `reported_genesis_hash` answers "what chain is
    /// this server serving". They agree wherever both have an answer; only the
    /// upstream chains differ, and there the report comes from zebra.
    #[test]
    fn the_pinned_and_the_reported_genesis_agree_where_both_exist() {
        for network in [custom_testnet(), swarm_mainnet()] {
            assert_eq!(
                network.expected_genesis_hash(),
                network.reported_genesis_hash(),
                "{network}",
            );
        }
        for network in [Network::Mainnet, Network::PubTestnet] {
            assert_eq!(network.expected_genesis_hash(), None, "{network}");
            assert!(network.reported_genesis_hash().is_some(), "{network}");
        }
        assert_eq!(Network::Regtest.expected_genesis_hash(), None);
        assert_eq!(Network::Regtest.reported_genesis_hash(), None);
    }

    /// The display name doubles as the `network_name` of the zebra parameters
    /// built during adoption, and zebra rejects reserved names, over-long names
    /// and anything outside `[A-Za-z0-9_]`. Checking it here fails at unit-test
    /// speed instead of at indexer startup.
    #[test]
    fn custom_testnet_display_name_is_accepted_by_zebra() {
        assert_eq!(custom_testnet().to_string(), CUSTOM_TESTNET_DISPLAY_NAME);
        zebra_chain::parameters::testnet::Parameters::build()
            .with_network_name(CUSTOM_TESTNET_DISPLAY_NAME)
            .expect("zebra must accept the custom testnet display name");
    }

    /// Zebra has no network kind for the SWARM production network, so a SWARM
    /// production address cannot become a zebra address at all. Without this the
    /// vendored conversion could quietly read `swm1…` as Mainnet.
    #[test]
    fn swarm_mainnet_addresses_have_no_zebra_network_kind() {
        use zcash_protocol::consensus::NetworkType;
        use zebra_chain::parameters::NetworkKind;
        use zebra_chain::primitives::{Address, UnsupportedNetworkType};

        assert_eq!(
            NetworkKind::try_from(NetworkType::SwarmMain),
            Err(UnsupportedNetworkType(NetworkType::SwarmMain)),
        );
        for (network_type, kind) in [
            (NetworkType::Main, NetworkKind::Mainnet),
            (NetworkType::Test, NetworkKind::Testnet),
            (NetworkType::Regtest, NetworkKind::Regtest),
        ] {
            assert_eq!(NetworkKind::try_from(network_type), Ok(kind));
            assert_eq!(NetworkType::from(kind), network_type);
        }

        // The shared crate parses the SWARM production encodings ...
        for encoded in [
            "s1MCkDhVejM4RqDyRR1rEJkudd26FVWipPD",
            "s3Mtm9Ez6HFNovPfrY7WpjPGZmYNxztrxbb",
        ] {
            let parsed: zcash_address::ZcashAddress =
                encoded.parse().expect("parses in zcash_address");
            // ... and zebra refuses to convert them into one of its own.
            assert!(
                parsed.convert::<Address>().is_err(),
                "{encoded} must not convert to a zebra address",
            );
        }
    }

    /// The published SwarmTestnet destinations keep converting as testnet addresses.
    #[test]
    fn swarm_testnet_destinations_still_convert() {
        use zebra_chain::parameters::NetworkKind;
        use zebra_chain::primitives::Address;

        for encoded in [
            "t2DGVURG5tAyXXSkj85JV5xbvTobYv7H99n",
            "t2LVPzRYpZ4QtRRmQMS1zWUmG7TZaYcMjBR",
            "t2UHhsicXnapNJrfewHqgwXef5HDwCHd7wk",
            "t2Li46A4YNFqRDvdKA212w7DtsLkbGMG2xU",
        ] {
            let parsed: zcash_address::ZcashAddress =
                encoded.parse().expect("parses in zcash_address");
            let converted = parsed
                .convert::<Address>()
                .expect("a SwarmTestnet destination is a testnet address");
            assert_eq!(converted.network(), NetworkKind::Testnet);
        }
    }

    #[test]
    fn activation_heights_round_trip_nu6_2() {
        let heights = ActivationHeights {
            before_overwinter: Some(1),
            overwinter: Some(1),
            sapling: Some(1),
            blossom: Some(1),
            heartwood: Some(1),
            canopy: Some(1),
            nu5: Some(1),
            nu6: Some(1),
            nu6_1: Some(1),
            nu6_2: Some(2),
            nu6_3: Some(500),
            nu7: Some(1000),
        };

        let zebra_heights: zebra_chain::parameters::testnet::ConfiguredActivationHeights =
            heights.into();
        assert_eq!(zebra_heights.nu6_2, Some(2));
    }
}

use std::{collections::BTreeMap, error::Error, fmt, future::Future, pin::Pin};

use schemars::{schema::RootSchema, JsonSchema};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

use crate::{
    model::{ActionConfig, Item},
    ActionRun,
};

pub type RawConfig = BTreeMap<String, Value>;
pub type ActionInputs = BTreeMap<String, String>;
pub type RuntimeResult<T> = anyhow::Result<T>;
pub type SourceFuture<'a> =
    Pin<Box<dyn Future<Output = RuntimeResult<SourceCollection>> + Send + 'a>>;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceEnvelope {
    pub sources: Vec<ConfiguredSourceEnvelope>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfiguredSourceEnvelope {
    pub id: String,
    pub source: SourceEnvelope,
    #[serde(default)]
    pub actions: Vec<ActionConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SourceEnvelope {
    pub kind: String,
    #[serde(flatten)]
    pub config: RawConfig,
}

#[derive(Debug)]
pub struct SourceCollection {
    pub items: Vec<Item>,
    pub available: Option<usize>,
    pub limit: usize,
}

pub struct SourceContext<'a> {
    pub source_id: &'a str,
}

pub struct ActionContext<'a> {
    pub workspace_id: &'a str,
    pub source_id: &'a str,
    pub item: &'a Item,
}

pub trait Source: Send + Sync {
    /// Collect Items. Errors belong to this Source; callers keep sibling Sources running.
    fn collect<'a>(&'a self, context: &'a SourceContext<'a>) -> SourceFuture<'a>;

    /// Check reachability. Defaults to collection so diagnostics retain collection metadata.
    fn health_check<'a>(&'a self, context: &'a SourceContext<'a>) -> SourceFuture<'a> {
        self.collect(context)
    }

    /// Return stable Store partition identity for this Source's Item universe.
    fn item_bucket_identity(&self) -> String;
}

pub trait Action: Send + Sync {
    /// Execute for one Item. Callers convert errors into failed, Item-scoped attempts.
    fn execute(&self, context: &ActionContext<'_>) -> RuntimeResult<ActionRun>;
}

pub trait SourceDefinition: Sized + 'static {
    const ID: &'static str;
    type Config: DeserializeOwned + JsonSchema + 'static;
    type Runtime: Source + 'static;

    /// Parse semantic invariants and construct runtime state without I/O or side effects.
    fn build(config: Self::Config) -> RuntimeResult<Self::Runtime>;
}

pub trait ActionDefinition: Sized + 'static {
    const ID: &'static str;
    type Config: DeserializeOwned + JsonSchema + 'static;
    type Runtime: Action + 'static;

    /// Parse semantic invariants and construct runtime state without I/O or side effects.
    fn build(config: Self::Config) -> RuntimeResult<Self::Runtime>;

    /// Check external execution requirements without constructing a per-Item runtime.
    fn health_check() -> RuntimeResult<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RegistryCategory {
    Source,
    Action,
}

#[derive(Debug)]
pub enum RegistryError {
    Duplicate {
        category: RegistryCategory,
        id: String,
    },
    Unknown {
        category: RegistryCategory,
        id: String,
    },
    InvalidConfig {
        category: RegistryCategory,
        id: String,
        source: serde_json::Error,
    },
    Factory {
        category: RegistryCategory,
        id: String,
        source: anyhow::Error,
    },
    Health {
        category: RegistryCategory,
        id: String,
        source: anyhow::Error,
    },
}

impl RegistryCategory {
    fn name(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Action => "action",
        }
    }
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate { category, id } => {
                write!(formatter, "duplicate {} registration {id}", category.name())
            }
            Self::Unknown { category, id } => {
                write!(formatter, "unknown {} registration {id}", category.name())
            }
            Self::InvalidConfig {
                category,
                id,
                source,
            } => write!(
                formatter,
                "invalid config for {} {id}: {source}",
                category.name()
            ),
            Self::Factory {
                category,
                id,
                source,
            } => write!(
                formatter,
                "{} {id} factory failed: {source}",
                category.name()
            ),
            Self::Health {
                category,
                id,
                source,
            } => write!(
                formatter,
                "{} {id} health check failed: {source}",
                category.name()
            ),
        }
    }
}

impl Error for RegistryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidConfig { source, .. } => Some(source),
            Self::Factory { source, .. } | Self::Health { source, .. } => Some(source.as_ref()),
            Self::Duplicate { .. } | Self::Unknown { .. } => None,
        }
    }
}

enum BuildError {
    InvalidConfig(serde_json::Error),
    Factory(anyhow::Error),
}

type SourceFactory = fn(RawConfig) -> Result<Box<dyn Source>, BuildError>;
type ActionValidator = fn(&ActionInputs) -> Result<(), serde_json::Error>;
type ActionFactory = fn(ActionInputs) -> Result<Box<dyn Action>, BuildError>;
type ActionHealthCheck = fn() -> RuntimeResult<()>;

pub struct SourceRegistration {
    id: &'static str,
    schema: RootSchema,
    factory: SourceFactory,
}

impl SourceRegistration {
    pub fn id(&self) -> &str {
        self.id
    }

    pub fn schema(&self) -> &RootSchema {
        &self.schema
    }
}

pub struct ActionRegistration {
    id: &'static str,
    schema: RootSchema,
    validate: ActionValidator,
    factory: ActionFactory,
    health_check: ActionHealthCheck,
}

impl ActionRegistration {
    pub fn id(&self) -> &str {
        self.id
    }

    pub fn schema(&self) -> &RootSchema {
        &self.schema
    }
}

#[derive(Default)]
pub struct Registry {
    sources: BTreeMap<&'static str, SourceRegistration>,
    actions: BTreeMap<&'static str, ActionRegistration>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_source<D: SourceDefinition>(&mut self) -> Result<(), RegistryError> {
        if self.sources.contains_key(D::ID) {
            return Err(RegistryError::Duplicate {
                category: RegistryCategory::Source,
                id: D::ID.to_string(),
            });
        }
        self.sources.insert(
            D::ID,
            SourceRegistration {
                id: D::ID,
                schema: schemars::schema_for!(D::Config),
                factory: build_source_definition::<D>,
            },
        );
        Ok(())
    }

    pub fn add_action<D: ActionDefinition>(&mut self) -> Result<(), RegistryError> {
        if self.actions.contains_key(D::ID) {
            return Err(RegistryError::Duplicate {
                category: RegistryCategory::Action,
                id: D::ID.to_string(),
            });
        }
        self.actions.insert(
            D::ID,
            ActionRegistration {
                id: D::ID,
                schema: schemars::schema_for!(D::Config),
                validate: validate_action_definition::<D>,
                factory: build_action_definition::<D>,
                health_check: D::health_check,
            },
        );
        Ok(())
    }

    pub fn sources(&self) -> impl Iterator<Item = &SourceRegistration> {
        self.sources.values()
    }

    pub fn actions(&self) -> impl Iterator<Item = &ActionRegistration> {
        self.actions.values()
    }

    pub fn build_source(
        &self,
        id: &str,
        config: RawConfig,
    ) -> Result<Box<dyn Source>, RegistryError> {
        let registration = self.sources.get(id).ok_or_else(|| RegistryError::Unknown {
            category: RegistryCategory::Source,
            id: id.to_string(),
        })?;
        (registration.factory)(config)
            .map_err(|error| registry_build_error(RegistryCategory::Source, id, error))
    }

    pub fn validate_action(&self, id: &str, inputs: &ActionInputs) -> Result<(), RegistryError> {
        let registration = self.actions.get(id).ok_or_else(|| RegistryError::Unknown {
            category: RegistryCategory::Action,
            id: id.to_string(),
        })?;
        (registration.validate)(inputs).map_err(|source| RegistryError::InvalidConfig {
            category: RegistryCategory::Action,
            id: id.to_string(),
            source,
        })
    }

    pub fn build_action(
        &self,
        id: &str,
        inputs: ActionInputs,
    ) -> Result<Box<dyn Action>, RegistryError> {
        let registration = self.actions.get(id).ok_or_else(|| RegistryError::Unknown {
            category: RegistryCategory::Action,
            id: id.to_string(),
        })?;
        (registration.factory)(inputs)
            .map_err(|error| registry_build_error(RegistryCategory::Action, id, error))
    }

    pub fn check_action(&self, id: &str) -> Result<(), RegistryError> {
        let registration = self.actions.get(id).ok_or_else(|| RegistryError::Unknown {
            category: RegistryCategory::Action,
            id: id.to_string(),
        })?;
        (registration.health_check)().map_err(|source| RegistryError::Health {
            category: RegistryCategory::Action,
            id: id.to_string(),
            source,
        })
    }
}

fn registry_build_error(category: RegistryCategory, id: &str, error: BuildError) -> RegistryError {
    match error {
        BuildError::InvalidConfig(source) => RegistryError::InvalidConfig {
            category,
            id: id.to_string(),
            source,
        },
        BuildError::Factory(source) => RegistryError::Factory {
            category,
            id: id.to_string(),
            source,
        },
    }
}

fn build_source_definition<D: SourceDefinition>(
    config: RawConfig,
) -> Result<Box<dyn Source>, BuildError> {
    let config = deserialize_raw(config).map_err(BuildError::InvalidConfig)?;
    D::build(config)
        .map(|runtime| Box::new(runtime) as Box<dyn Source>)
        .map_err(BuildError::Factory)
}

fn validate_action_definition<D: ActionDefinition>(
    inputs: &ActionInputs,
) -> Result<(), serde_json::Error> {
    deserialize_action::<D::Config>(inputs.clone()).map(drop)
}

fn build_action_definition<D: ActionDefinition>(
    inputs: ActionInputs,
) -> Result<Box<dyn Action>, BuildError> {
    let config = deserialize_action(inputs).map_err(BuildError::InvalidConfig)?;
    D::build(config)
        .map(|runtime| Box::new(runtime) as Box<dyn Action>)
        .map_err(BuildError::Factory)
}

fn deserialize_raw<T: DeserializeOwned>(config: RawConfig) -> Result<T, serde_json::Error> {
    serde_json::from_value(Value::Object(config.into_iter().collect()))
}

fn deserialize_action<T: DeserializeOwned>(inputs: ActionInputs) -> Result<T, serde_json::Error> {
    serde_json::from_value(Value::Object(
        inputs
            .into_iter()
            .map(|(key, value)| (key, Value::String(value)))
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use anyhow::bail;
    use schemars::JsonSchema;
    use serde::Deserialize;
    use serde_json::json;

    use super::*;

    static SOURCE_BUILDS: AtomicUsize = AtomicUsize::new(0);
    static ACTION_BUILDS: AtomicUsize = AtomicUsize::new(0);

    #[derive(Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    struct TestSourceConfig {
        bucket: String,
    }

    struct TestSource {
        bucket: String,
    }

    impl Source for TestSource {
        fn collect<'a>(&'a self, _context: &'a SourceContext<'a>) -> SourceFuture<'a> {
            Box::pin(async {
                Ok(SourceCollection {
                    items: Vec::new(),
                    available: None,
                    limit: 5,
                })
            })
        }

        fn item_bucket_identity(&self) -> String {
            self.bucket.clone()
        }
    }

    struct TestSourceDefinition;

    impl SourceDefinition for TestSourceDefinition {
        const ID: &'static str = "test/source";
        type Config = TestSourceConfig;
        type Runtime = TestSource;

        fn build(config: Self::Config) -> RuntimeResult<Self::Runtime> {
            Ok(TestSource {
                bucket: config.bucket,
            })
        }
    }

    struct DuplicateSourceDefinition;

    impl SourceDefinition for DuplicateSourceDefinition {
        const ID: &'static str = TestSourceDefinition::ID;
        type Config = TestSourceConfig;
        type Runtime = TestSource;

        fn build(config: Self::Config) -> RuntimeResult<Self::Runtime> {
            Ok(TestSource {
                bucket: config.bucket,
            })
        }
    }

    struct AlphaSourceDefinition;
    struct ZetaSourceDefinition;

    macro_rules! source_definition {
        ($definition:ty, $id:literal) => {
            impl SourceDefinition for $definition {
                const ID: &'static str = $id;
                type Config = TestSourceConfig;
                type Runtime = TestSource;

                fn build(config: Self::Config) -> RuntimeResult<Self::Runtime> {
                    Ok(TestSource {
                        bucket: config.bucket,
                    })
                }
            }
        };
    }

    source_definition!(AlphaSourceDefinition, "alpha");
    source_definition!(ZetaSourceDefinition, "zeta");

    struct FailingSourceDefinition;

    impl SourceDefinition for FailingSourceDefinition {
        const ID: &'static str = "failing/source";
        type Config = TestSourceConfig;
        type Runtime = TestSource;

        fn build(_config: Self::Config) -> RuntimeResult<Self::Runtime> {
            bail!("source factory failed")
        }
    }

    #[derive(Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    struct TestActionConfig {
        command: String,
    }

    struct TestAction {
        command: String,
    }

    impl Action for TestAction {
        fn execute(&self, _context: &ActionContext<'_>) -> RuntimeResult<ActionRun> {
            Ok(ActionRun {
                success: true,
                stdout: self.command.clone(),
                stderr: String::new(),
                message: None,
            })
        }
    }

    struct TestActionDefinition;

    impl ActionDefinition for TestActionDefinition {
        const ID: &'static str = "test/action";
        type Config = TestActionConfig;
        type Runtime = TestAction;

        fn build(config: Self::Config) -> RuntimeResult<Self::Runtime> {
            Ok(TestAction {
                command: config.command,
            })
        }
    }

    struct DuplicateActionDefinition;

    impl ActionDefinition for DuplicateActionDefinition {
        const ID: &'static str = TestActionDefinition::ID;
        type Config = TestActionConfig;
        type Runtime = TestAction;

        fn build(config: Self::Config) -> RuntimeResult<Self::Runtime> {
            Ok(TestAction {
                command: config.command,
            })
        }
    }

    struct AlphaActionDefinition;
    struct ZetaActionDefinition;

    macro_rules! action_definition {
        ($definition:ty, $id:literal) => {
            impl ActionDefinition for $definition {
                const ID: &'static str = $id;
                type Config = TestActionConfig;
                type Runtime = TestAction;

                fn build(config: Self::Config) -> RuntimeResult<Self::Runtime> {
                    Ok(TestAction {
                        command: config.command,
                    })
                }
            }
        };
    }

    action_definition!(AlphaActionDefinition, "alpha");
    action_definition!(ZetaActionDefinition, "zeta");

    struct FailingActionDefinition;

    impl ActionDefinition for FailingActionDefinition {
        const ID: &'static str = "failing/action";
        type Config = TestActionConfig;
        type Runtime = TestAction;

        fn build(_config: Self::Config) -> RuntimeResult<Self::Runtime> {
            bail!("action factory failed")
        }
    }

    struct FailingHealthActionDefinition;

    impl ActionDefinition for FailingHealthActionDefinition {
        const ID: &'static str = "failing/health";
        type Config = TestActionConfig;
        type Runtime = TestAction;

        fn build(config: Self::Config) -> RuntimeResult<Self::Runtime> {
            Ok(TestAction {
                command: config.command,
            })
        }

        fn health_check() -> RuntimeResult<()> {
            bail!("action health failed")
        }
    }

    struct SharedSourceDefinition;
    struct SharedActionDefinition;
    source_definition!(SharedSourceDefinition, "shared");
    action_definition!(SharedActionDefinition, "shared");

    struct MetadataSourceDefinition;

    impl SourceDefinition for MetadataSourceDefinition {
        const ID: &'static str = "metadata/source";
        type Config = TestSourceConfig;
        type Runtime = TestSource;

        fn build(config: Self::Config) -> RuntimeResult<Self::Runtime> {
            SOURCE_BUILDS.fetch_add(1, Ordering::SeqCst);
            Ok(TestSource {
                bucket: config.bucket,
            })
        }
    }

    struct MetadataActionDefinition;

    impl ActionDefinition for MetadataActionDefinition {
        const ID: &'static str = "metadata/action";
        type Config = TestActionConfig;
        type Runtime = TestAction;

        fn build(config: Self::Config) -> RuntimeResult<Self::Runtime> {
            ACTION_BUILDS.fetch_add(1, Ordering::SeqCst);
            Ok(TestAction {
                command: config.command,
            })
        }
    }

    fn raw_config(value: Value) -> RawConfig {
        serde_json::from_value(value).unwrap()
    }

    fn action_inputs(command: &str) -> ActionInputs {
        BTreeMap::from([("command".to_string(), command.to_string())])
    }

    fn sample_item() -> Item {
        Item {
            id: "item-1".to_string(),
            reference_id: "AB-1".to_string(),
            title: "Test item".to_string(),
            status: "ready".to_string(),
            url: "https://example.test/items/1".to_string(),
            source_id: "source-1".to_string(),
            source_kind: "test".to_string(),
            raw: json!({"id": 1}),
        }
    }

    fn poll_source(mut future: SourceFuture<'_>) -> RuntimeResult<SourceCollection> {
        let waker = std::task::Waker::noop();
        let mut context = std::task::Context::from_waker(waker);
        match std::future::Future::poll(future.as_mut(), &mut context) {
            std::task::Poll::Ready(result) => result,
            std::task::Poll::Pending => panic!("test Source future unexpectedly pending"),
        }
    }

    #[test]
    fn rejects_duplicate_source_registration() {
        let mut registry = Registry::new();
        registry.add_source::<TestSourceDefinition>().unwrap();

        let error = registry
            .add_source::<DuplicateSourceDefinition>()
            .unwrap_err();

        assert!(matches!(
            error,
            RegistryError::Duplicate {
                category: RegistryCategory::Source,
                ref id,
            } if id == TestSourceDefinition::ID
        ));
    }

    #[test]
    fn rejects_duplicate_action_registration() {
        let mut registry = Registry::new();
        registry.add_action::<TestActionDefinition>().unwrap();

        let error = registry
            .add_action::<DuplicateActionDefinition>()
            .unwrap_err();

        assert!(matches!(
            error,
            RegistryError::Duplicate {
                category: RegistryCategory::Action,
                ref id,
            } if id == TestActionDefinition::ID
        ));
    }

    #[test]
    fn keeps_source_and_action_namespaces_separate() {
        let mut registry = Registry::new();
        registry.add_source::<SharedSourceDefinition>().unwrap();
        registry.add_action::<SharedActionDefinition>().unwrap();

        let source = registry
            .build_source("shared", raw_config(json!({"bucket": "items"})))
            .unwrap();
        let action = registry
            .build_action("shared", action_inputs("echo test"))
            .unwrap();
        let source_context = SourceContext {
            source_id: "source-1",
        };
        let collection = poll_source(source.health_check(&source_context)).unwrap();
        let item = sample_item();
        let run = action
            .execute(&ActionContext {
                workspace_id: "workspace-1",
                source_id: "source-1",
                item: &item,
            })
            .unwrap();

        registry.check_action("shared").unwrap();
        assert_eq!(registry.sources().next().unwrap().id(), "shared");
        assert_eq!(registry.actions().next().unwrap().id(), "shared");
        assert_eq!(source.item_bucket_identity(), "items");
        assert_eq!(collection.limit, 5);
        assert_eq!(collection.available, None);
        assert!(collection.items.is_empty());
        assert_eq!(run.stdout, "echo test");
    }

    #[test]
    fn iterates_registrations_deterministically() {
        let mut registry = Registry::new();
        registry.add_source::<ZetaSourceDefinition>().unwrap();
        registry.add_source::<AlphaSourceDefinition>().unwrap();
        registry.add_action::<ZetaActionDefinition>().unwrap();
        registry.add_action::<AlphaActionDefinition>().unwrap();

        assert_eq!(
            registry
                .sources()
                .map(SourceRegistration::id)
                .collect::<Vec<_>>(),
            ["alpha", "zeta"]
        );
        assert_eq!(
            registry
                .actions()
                .map(ActionRegistration::id)
                .collect::<Vec<_>>(),
            ["alpha", "zeta"]
        );
    }

    #[test]
    fn reports_unknown_ids_by_category() {
        let registry = Registry::new();

        assert!(matches!(
            registry.build_source("missing", RawConfig::new()),
            Err(RegistryError::Unknown {
                category: RegistryCategory::Source,
                ref id,
            }) if id == "missing"
        ));
        assert!(matches!(
            registry.validate_action("missing", &ActionInputs::new()),
            Err(RegistryError::Unknown {
                category: RegistryCategory::Action,
                ref id,
            }) if id == "missing"
        ));
        assert!(matches!(
            registry.build_action("missing", ActionInputs::new()),
            Err(RegistryError::Unknown {
                category: RegistryCategory::Action,
                ref id,
            }) if id == "missing"
        ));
        assert!(matches!(
            registry.check_action("missing"),
            Err(RegistryError::Unknown {
                category: RegistryCategory::Action,
                ref id,
            }) if id == "missing"
        ));
    }

    #[test]
    fn rejects_invalid_typed_config() {
        let mut registry = Registry::new();
        registry.add_source::<TestSourceDefinition>().unwrap();
        registry.add_action::<TestActionDefinition>().unwrap();

        assert!(matches!(
            registry.build_source(TestSourceDefinition::ID, raw_config(json!({"extra": true}))),
            Err(RegistryError::InvalidConfig {
                category: RegistryCategory::Source,
                ..
            })
        ));
        assert!(matches!(
            registry.validate_action(
                TestActionDefinition::ID,
                &BTreeMap::from([("extra".to_string(), "value".to_string())]),
            ),
            Err(RegistryError::InvalidConfig {
                category: RegistryCategory::Action,
                ..
            })
        ));
    }

    #[test]
    fn propagates_factory_errors_with_category_and_id() {
        let mut registry = Registry::new();
        registry.add_source::<FailingSourceDefinition>().unwrap();
        registry.add_action::<FailingActionDefinition>().unwrap();

        let source_error = match registry.build_source(
            FailingSourceDefinition::ID,
            raw_config(json!({"bucket": "items"})),
        ) {
            Ok(_) => panic!("source factory unexpectedly succeeded"),
            Err(error) => error,
        };
        let action_error =
            match registry.build_action(FailingActionDefinition::ID, action_inputs("echo test")) {
                Ok(_) => panic!("action factory unexpectedly succeeded"),
                Err(error) => error,
            };

        assert!(matches!(
            source_error,
            RegistryError::Factory {
                category: RegistryCategory::Source,
                ref id,
                ..
            } if id == FailingSourceDefinition::ID
        ));
        assert!(matches!(
            action_error,
            RegistryError::Factory {
                category: RegistryCategory::Action,
                ref id,
                ..
            } if id == FailingActionDefinition::ID
        ));
        assert!(source_error.to_string().contains("source factory failed"));
        assert!(action_error.to_string().contains("action factory failed"));
        assert_eq!(
            std::error::Error::source(&source_error)
                .unwrap()
                .to_string(),
            "source factory failed"
        );
        assert_eq!(
            std::error::Error::source(&action_error)
                .unwrap()
                .to_string(),
            "action factory failed"
        );
    }

    #[test]
    fn propagates_action_health_errors_with_category_and_id() {
        let mut registry = Registry::new();
        registry
            .add_action::<FailingHealthActionDefinition>()
            .unwrap();

        let error = registry
            .check_action(FailingHealthActionDefinition::ID)
            .unwrap_err();

        assert!(matches!(
            error,
            RegistryError::Health {
                category: RegistryCategory::Action,
                ref id,
                ..
            } if id == FailingHealthActionDefinition::ID
        ));
        assert_eq!(
            std::error::Error::source(&error).unwrap().to_string(),
            "action health failed"
        );
    }

    #[test]
    fn exposes_ids_and_schemas_without_constructing_runtimes() {
        SOURCE_BUILDS.store(0, Ordering::SeqCst);
        ACTION_BUILDS.store(0, Ordering::SeqCst);
        let mut registry = Registry::new();
        registry.add_source::<MetadataSourceDefinition>().unwrap();
        registry.add_action::<MetadataActionDefinition>().unwrap();

        let source = registry.sources().next().unwrap();
        let action = registry.actions().next().unwrap();

        assert_eq!(source.id(), MetadataSourceDefinition::ID);
        assert_eq!(action.id(), MetadataActionDefinition::ID);
        assert!(serde_json::to_value(source.schema()).unwrap()["required"].is_array());
        assert!(serde_json::to_value(action.schema()).unwrap()["required"].is_array());
        assert_eq!(SOURCE_BUILDS.load(Ordering::SeqCst), 0);
        assert_eq!(ACTION_BUILDS.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn workspace_envelope_preserves_existing_toml_field_names() {
        let text = r#"
[[sources]]
id = "local"

[sources.source]
kind = "qmd"
query = "status:ready"

[[sources.actions]]
uses = "agentboard/run-cmd"

[sources.actions.with]
command = "echo {{ item.id }}"
"#;

        let envelope: WorkspaceEnvelope = toml::from_str(text).unwrap();
        let serialized = toml::to_string(&envelope).unwrap();
        let round_trip: WorkspaceEnvelope = toml::from_str(&serialized).unwrap();

        assert!(serialized.contains("kind = \"qmd\""));
        assert!(serialized.contains("uses = \"agentboard/run-cmd\""));
        assert_eq!(round_trip.sources[0].source.kind, "qmd");
        assert_eq!(round_trip.sources[0].source.config["query"], "status:ready");
        assert_eq!(round_trip.sources[0].actions[0].uses, "agentboard/run-cmd");
        assert_eq!(
            round_trip.sources[0].actions[0].inputs["command"],
            "echo {{ item.id }}"
        );
    }
}

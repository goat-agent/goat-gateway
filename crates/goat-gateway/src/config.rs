use std::path::Path;

use toml_edit::{DocumentMut, Item, Table};

use crate::provider::{Catalog, CatalogError};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error(
        "{0:?} cannot name a provider. Use lowercase letters, digits, - and _, starting with a letter"
    )]
    BadName(String),
    #[error("config.toml is not valid TOML, so it was left alone: {0}")]
    Unreadable(String),
    #[error("the declaration is not valid TOML: {0}")]
    Declaration(String),
    #[error(transparent)]
    Rejected(#[from] CatalogError),
    #[error("could not write {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{0:?} is not declared in config.toml, so there is nothing to remove")]
    Absent(String),
}

pub fn named(id: &str) -> Result<(), ConfigError> {
    let mut characters = id.chars();
    let starts_well = characters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase());
    let rest_fits =
        characters.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if starts_well && rest_fits && id.len() <= 48 {
        Ok(())
    } else {
        Err(ConfigError::BadName(id.to_owned()))
    }
}

pub fn declared(existing: &str, id: &str, declaration: &str) -> Result<String, ConfigError> {
    named(id)?;
    let mut document = parse(existing)?;
    let body = declaration
        .parse::<DocumentMut>()
        .map_err(|error| ConfigError::Declaration(error.to_string()))?;

    let providers = providers(&mut document);
    let mut table: Table = body.as_table().clone();
    table.set_implicit(false);
    providers.insert(id, Item::Table(table));

    let written = document.to_string();
    Catalog::with_overlay(&written)?;
    Ok(written)
}

pub fn undeclared(existing: &str, id: &str) -> Result<String, ConfigError> {
    let mut document = parse(existing)?;
    let removed = providers(&mut document).remove(id);
    if removed.is_none() {
        return Err(ConfigError::Absent(id.to_owned()));
    }
    Ok(document.to_string())
}

pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

pub fn write(path: &Path, contents: &str) -> Result<(), ConfigError> {
    let failed = |source| ConfigError::Write {
        path: path.display().to_string(),
        source,
    };
    let staged = path.with_extension("toml.writing");
    std::fs::write(&staged, contents).map_err(failed)?;
    std::fs::rename(&staged, path).map_err(failed)
}

fn parse(existing: &str) -> Result<DocumentMut, ConfigError> {
    existing
        .parse::<DocumentMut>()
        .map_err(|error| ConfigError::Unreadable(error.to_string()))
}

fn providers(document: &mut DocumentMut) -> &mut Table {
    let entry = document
        .as_table_mut()
        .entry("provider")
        .or_insert_with(|| {
            let mut table = Table::new();
            table.set_implicit(true);
            Item::Table(table)
        });
    if !entry.is_table() {
        let mut table = Table::new();
        table.set_implicit(true);
        *entry = Item::Table(table);
    }
    entry
        .as_table_mut()
        .expect("provider was just made a table")
}

#[cfg(test)]
mod tests {
    use super::*;

    const DECLARATION: &str = r#"
label = "Office proxy"
key = "bearer"
endpoints = [{ wire = "chat", url = "https://llm.office.example/v1/chat/completions" }]

[[models]]
name = "office-large"
"#;

    #[test]
    fn a_declaration_lands_under_its_name_and_the_hand_written_rest_survives() {
        let existing = "# our providers, edited by hand\n\n[provider.kimi]\n# keep the plan host\nlabel = \"Kimi at work\"\nendpoints = [{ wire = \"chat\", url = \"https://api.kimi.com/coding/v1/chat/completions\" }]\n";
        let written = declared(existing, "office", DECLARATION).unwrap();

        assert!(written.contains("# our providers, edited by hand"));
        assert!(written.contains("# keep the plan host"));
        assert!(written.contains("[provider.office]"));
        assert!(written.contains("[[provider.office.models]]"));

        let catalog = Catalog::with_overlay(&written).unwrap();
        assert_eq!(catalog.get("office").unwrap().label, "Office proxy");
        assert_eq!(catalog.get("kimi").unwrap().label, "Kimi at work");
    }

    #[test]
    fn a_declaration_the_catalog_would_refuse_is_never_written() {
        let error =
            declared("", "broken", "label = \"No endpoints\"\nendpoints = []\n").unwrap_err();
        assert!(matches!(error, ConfigError::Rejected(_)), "{error}");

        let typo = declared("", "typo", "labl = \"x\"\n").unwrap_err();
        assert!(matches!(typo, ConfigError::Rejected(_)), "{typo}");
    }

    #[test]
    fn a_name_that_cannot_be_a_table_key_is_refused_before_anything_is_parsed() {
        for bad in ["", "Office", "has space", "9lives", "a.b"] {
            assert!(matches!(
                declared("", bad, DECLARATION),
                Err(ConfigError::BadName(_))
            ));
        }
    }

    #[test]
    fn replacing_a_declaration_replaces_it_whole() {
        let first = declared("", "office", DECLARATION).unwrap();
        let second = declared(
            &first,
            "office",
            "label = \"Renamed\"\nendpoints = [{ wire = \"messages\", url = \"https://llm.office.example/v1/messages\" }]\n",
        )
        .unwrap();
        let catalog = Catalog::with_overlay(&second).unwrap();
        let office = catalog.get("office").unwrap();
        assert_eq!(office.label, "Renamed");
        assert!(office.models.is_empty(), "the old models must not linger");
    }

    #[test]
    fn removing_a_declaration_leaves_the_others() {
        let both = declared(
            &declared("", "office", DECLARATION).unwrap(),
            "lab",
            "label = \"Lab\"\nendpoints = [{ wire = \"chat\", url = \"http://10.0.0.9/v1/chat/completions\" }]\n",
        )
        .unwrap();
        let one = undeclared(&both, "office").unwrap();
        let catalog = Catalog::with_overlay(&one).unwrap();
        assert!(catalog.get("office").is_none());
        assert!(catalog.get("lab").is_some());
        assert!(matches!(
            undeclared(&one, "office"),
            Err(ConfigError::Absent(_))
        ));
    }

    #[test]
    fn a_config_file_that_does_not_parse_is_left_alone() {
        let error = declared("[provider\nbroken", "office", DECLARATION).unwrap_err();
        assert!(matches!(error, ConfigError::Unreadable(_)));
    }
}

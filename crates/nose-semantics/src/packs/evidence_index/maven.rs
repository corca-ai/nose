use super::{SemanticPackDependencyEvidence, SemanticPackDependencyEvidenceId};
use crate::{
    SemanticPackDependencySource, SemanticPackV1Authorization, SemanticPackV1PackageEcosystem,
    MAX_SEMANTIC_PACK_DEPENDENCY_BYTES,
};
use quick_xml::events::Event;
use quick_xml::Reader;
use semver::{Version, VersionReq};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

const MAX_XML_DEPTH: usize = 64;
const MAX_PROPERTY_EXPANSIONS: usize = 16;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) struct MavenEvidenceKey {
    coordinate: String,
    declared_version: String,
    matched_version: String,
    sources: Vec<SemanticPackDependencySource>,
}

impl MavenEvidenceKey {
    pub(super) fn to_evidence(
        &self,
        id: SemanticPackDependencyEvidenceId,
    ) -> SemanticPackDependencyEvidence {
        SemanticPackDependencyEvidence {
            id,
            ecosystem: SemanticPackV1PackageEcosystem::Maven,
            coordinate: self.coordinate.clone(),
            declared_version: self.declared_version.clone(),
            matched_version: self.matched_version.clone(),
            sources: self.sources.clone(),
        }
    }
}

pub(super) enum MavenResolution {
    Missing,
    InvalidVersion,
    AmbiguousVersion,
    OutOfRange,
    Resolved(MavenEvidenceKey),
}

#[derive(Default)]
pub(super) struct MavenCatalog {
    declarations: BTreeMap<String, Vec<MavenDeclaration>>,
}

impl MavenCatalog {
    pub(super) fn from_authorizations<'a>(
        authorizations: impl Iterator<Item = &'a SemanticPackV1Authorization>,
    ) -> Self {
        let mut files = BTreeMap::new();
        for authorization in authorizations {
            for file in authorization.dependencies() {
                files
                    .entry((file.declared_path(), file.content_digest()))
                    .or_insert(file);
            }
        }
        let mut catalog = Self::default();
        for file in files.values() {
            catalog.read_file(file);
        }
        catalog
    }

    pub(super) fn from_conformance_paths(paths: &[std::path::PathBuf]) -> Self {
        let mut catalog = Self::default();
        for path in paths {
            let Ok(bytes) = fs::read(path) else { continue };
            let content_digest = digest(&bytes);
            catalog.read_bytes(&bytes, &path.to_string_lossy(), &content_digest);
        }
        catalog
    }

    pub(super) fn resolve(&self, coordinate: &str, requirement: &str) -> MavenResolution {
        let Some(declarations) = self.declarations.get(coordinate) else {
            return MavenResolution::Missing;
        };
        let mut versions = BTreeMap::<String, Vec<SemanticPackDependencySource>>::new();
        let mut has_invalid = false;
        for declaration in declarations {
            let Some(version) = &declaration.version else {
                has_invalid = true;
                continue;
            };
            versions
                .entry(version.clone())
                .or_default()
                .push(declaration.source.clone());
        }
        if has_invalid || versions.is_empty() {
            return MavenResolution::InvalidVersion;
        }
        if versions.len() != 1 {
            return MavenResolution::AmbiguousVersion;
        }
        let (declared_version, mut sources) = versions.into_iter().next().expect("one version");
        let Some(matched_version) = maven_release_version(&declared_version) else {
            return MavenResolution::InvalidVersion;
        };
        let normalized_requirement = requirement
            .replace(',', " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(",");
        let Ok(requirement) = VersionReq::parse(&normalized_requirement) else {
            return MavenResolution::InvalidVersion;
        };
        if !requirement.matches(&matched_version) {
            return MavenResolution::OutOfRange;
        }
        sources.sort();
        sources.dedup();
        MavenResolution::Resolved(MavenEvidenceKey {
            coordinate: coordinate.to_string(),
            declared_version,
            matched_version: matched_version.to_string(),
            sources,
        })
    }

    fn read_file(&mut self, file: &crate::SemanticPackLockedFile) {
        let Ok(bytes) = fs::read(file.resolved_path()) else {
            return;
        };
        if digest(&bytes) != file.content_digest() {
            return;
        }
        self.read_bytes(&bytes, file.declared_path(), file.content_digest());
    }

    fn read_bytes(&mut self, bytes: &[u8], declared_path: &str, content_digest: &str) {
        if bytes.len() > MAX_SEMANTIC_PACK_DEPENDENCY_BYTES {
            return;
        }
        let Ok(text) = std::str::from_utf8(bytes) else {
            return;
        };
        let Ok(pom) = parse_pom(text) else {
            return;
        };
        let source = SemanticPackDependencySource {
            declared_path: declared_path.to_string(),
            content_digest: content_digest.to_string(),
        };
        for dependency in pom.direct_dependencies() {
            self.declarations
                .entry(dependency.coordinate)
                .or_default()
                .push(MavenDeclaration {
                    version: dependency.version,
                    source: source.clone(),
                });
        }
    }
}

#[derive(Clone)]
struct MavenDeclaration {
    version: Option<String>,
    source: SemanticPackDependencySource,
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn maven_release_version(raw: &str) -> Option<Version> {
    if let Ok(version) = Version::parse(raw) {
        if version.pre.is_empty() {
            return Some(version);
        }
    }
    let core = raw
        .strip_suffix("-jre")
        .or_else(|| raw.strip_suffix("-android"))?;
    let version = Version::parse(core).ok()?;
    (version.pre.is_empty() && version.build.is_empty()).then_some(version)
}

#[derive(Default)]
struct MavenPom {
    project_fields: BTreeMap<String, Vec<String>>,
    parent_fields: BTreeMap<String, Vec<String>>,
    properties: BTreeMap<String, Vec<String>>,
    managed: Vec<RawDependency>,
    direct: Vec<RawDependency>,
}

impl MavenPom {
    fn direct_dependencies(&self) -> Vec<ResolvedDependency> {
        let properties = self.resolved_properties();
        let managed = self.managed_versions(&properties);
        self.direct
            .iter()
            .filter_map(|dependency| {
                let group = resolve_field(&dependency.fields, "groupId", &properties)?;
                let artifact = resolve_field(&dependency.fields, "artifactId", &properties)?;
                let coordinate = format!("{group}:{artifact}");
                let version = resolve_field(&dependency.fields, "version", &properties)
                    .or_else(|| managed.get(&coordinate).cloned());
                Some(ResolvedDependency {
                    coordinate,
                    version,
                })
            })
            .collect()
    }

    fn resolved_properties(&self) -> BTreeMap<String, String> {
        let mut values = BTreeMap::new();
        for (key, candidates) in &self.properties {
            if let Some(value) = unique(candidates) {
                values.insert(key.clone(), value.to_string());
            }
        }
        let group = unique_field(&self.project_fields, "groupId")
            .or_else(|| unique_field(&self.parent_fields, "groupId"));
        let version = unique_field(&self.project_fields, "version")
            .or_else(|| unique_field(&self.parent_fields, "version"));
        let artifact = unique_field(&self.project_fields, "artifactId");
        insert_project_property(&mut values, "groupId", group);
        insert_project_property(&mut values, "version", version);
        insert_project_property(&mut values, "artifactId", artifact);
        for _ in 0..MAX_PROPERTY_EXPANSIONS {
            let previous = values.clone();
            for value in values.values_mut() {
                if let Some(expanded) = expand_properties(value, &previous) {
                    *value = expanded;
                }
            }
            if values == previous {
                break;
            }
        }
        values
    }

    fn managed_versions(&self, properties: &BTreeMap<String, String>) -> BTreeMap<String, String> {
        let mut versions = BTreeMap::<String, BTreeSet<String>>::new();
        for dependency in &self.managed {
            let Some(group) = resolve_field(&dependency.fields, "groupId", properties) else {
                continue;
            };
            let Some(artifact) = resolve_field(&dependency.fields, "artifactId", properties) else {
                continue;
            };
            let Some(version) = resolve_field(&dependency.fields, "version", properties) else {
                continue;
            };
            versions
                .entry(format!("{group}:{artifact}"))
                .or_default()
                .insert(version);
        }
        versions
            .into_iter()
            .filter_map(|(coordinate, values)| {
                (values.len() == 1).then(|| (coordinate, values.into_iter().next().unwrap()))
            })
            .collect()
    }
}

fn insert_project_property(
    properties: &mut BTreeMap<String, String>,
    field: &str,
    value: Option<&str>,
) {
    let Some(value) = value else { return };
    properties.insert(format!("project.{field}"), value.to_string());
    properties.insert(format!("pom.{field}"), value.to_string());
}

struct ResolvedDependency {
    coordinate: String,
    version: Option<String>,
}

#[derive(Default)]
struct RawDependency {
    fields: BTreeMap<String, Vec<String>>,
}

struct XmlFrame {
    name: String,
    text: String,
}

struct ActiveDependency {
    depth: usize,
    managed: bool,
    dependency: RawDependency,
}

fn parse_pom(xml: &str) -> Result<MavenPom, ()> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut frames = Vec::<XmlFrame>::new();
    let mut pom = MavenPom::default();
    let mut active = None::<ActiveDependency>;
    loop {
        match reader.read_event().map_err(|_| ())? {
            Event::Start(start) => {
                let name = std::str::from_utf8(start.local_name().as_ref())
                    .map_err(|_| ())?
                    .to_string();
                frames.push(XmlFrame {
                    name,
                    text: String::new(),
                });
                if frames.len() > MAX_XML_DEPTH {
                    return Err(());
                }
                let path = path(&frames);
                if path == ["project", "dependencies", "dependency"] {
                    active = Some(ActiveDependency {
                        depth: frames.len(),
                        managed: false,
                        dependency: RawDependency::default(),
                    });
                } else if path
                    == [
                        "project",
                        "dependencyManagement",
                        "dependencies",
                        "dependency",
                    ]
                {
                    active = Some(ActiveDependency {
                        depth: frames.len(),
                        managed: true,
                        dependency: RawDependency::default(),
                    });
                }
            }
            Event::Text(text) => {
                let decoded = text.decode().map_err(|_| ())?;
                if let Some(frame) = frames.last_mut() {
                    frame.text.push_str(&decoded);
                }
            }
            Event::CData(text) => {
                let decoded = text.decode().map_err(|_| ())?;
                if let Some(frame) = frames.last_mut() {
                    frame.text.push_str(&decoded);
                }
            }
            Event::End(_) => close_frame(&mut frames, &mut active, &mut pom)?,
            Event::DocType(_) => return Err(()),
            Event::Eof => break,
            Event::Decl(_)
            | Event::Comment(_)
            | Event::PI(_)
            | Event::Empty(_)
            | Event::GeneralRef(_) => {}
        }
    }
    if !frames.is_empty() || active.is_some() {
        return Err(());
    }
    Ok(pom)
}

fn close_frame(
    frames: &mut Vec<XmlFrame>,
    active: &mut Option<ActiveDependency>,
    pom: &mut MavenPom,
) -> Result<(), ()> {
    let path = frames
        .iter()
        .map(|frame| frame.name.clone())
        .collect::<Vec<_>>();
    let frame = frames.pop().ok_or(())?;
    let value = frame.text.trim();
    if let Some(dependency) = active.as_mut() {
        if frames.len() + 1 == dependency.depth + 1 && !value.is_empty() {
            dependency
                .dependency
                .fields
                .entry(frame.name.clone())
                .or_default()
                .push(value.to_string());
        }
        if frames.len() + 1 == dependency.depth {
            let dependency = active.take().expect("active dependency");
            if dependency.managed {
                pom.managed.push(dependency.dependency);
            } else {
                pom.direct.push(dependency.dependency);
            }
        }
        return Ok(());
    }
    if path.len() == 2 && path[0] == "project" && !value.is_empty() {
        pom.project_fields
            .entry(frame.name)
            .or_default()
            .push(value.to_string());
    } else if path.len() == 3 && path[..2] == ["project", "parent"] && !value.is_empty() {
        pom.parent_fields
            .entry(frame.name)
            .or_default()
            .push(value.to_string());
    } else if path.len() == 3 && path[..2] == ["project", "properties"] && !value.is_empty() {
        pom.properties
            .entry(frame.name)
            .or_default()
            .push(value.to_string());
    }
    Ok(())
}

fn path(frames: &[XmlFrame]) -> Vec<&str> {
    frames.iter().map(|frame| frame.name.as_str()).collect()
}

fn resolve_field(
    fields: &BTreeMap<String, Vec<String>>,
    name: &str,
    properties: &BTreeMap<String, String>,
) -> Option<String> {
    let raw = unique(fields.get(name)?)?;
    let resolved = expand_properties(raw, properties)?;
    (!resolved.is_empty()).then_some(resolved)
}

fn unique_field<'a>(fields: &'a BTreeMap<String, Vec<String>>, name: &str) -> Option<&'a str> {
    unique(fields.get(name)?)
}

fn unique(values: &[String]) -> Option<&str> {
    let first = values.first()?;
    values.iter().all(|value| value == first).then_some(first)
}

fn expand_properties(raw: &str, properties: &BTreeMap<String, String>) -> Option<String> {
    let mut output = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find("${") {
        output.push_str(&rest[..start]);
        let tail = &rest[start + 2..];
        let end = tail.find('}')?;
        let name = &tail[..end];
        output.push_str(properties.get(name)?);
        rest = &tail[end + 1..];
        if output.len() > 4096 {
            return None;
        }
    }
    output.push_str(rest);
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_direct_and_managed_property_versions_but_not_profiles() {
        let pom = parse_pom(
            r#"<project>
              <properties><guava.version>33.0.0-jre</guava.version></properties>
              <dependencyManagement><dependencies><dependency>
                <groupId>com.google.guava</groupId><artifactId>guava</artifactId>
                <version>${guava.version}</version>
              </dependency></dependencies></dependencyManagement>
              <dependencies><dependency>
                <groupId>com.google.guava</groupId><artifactId>guava</artifactId>
              </dependency></dependencies>
              <profiles><profile><dependencies><dependency>
                <groupId>hidden</groupId><artifactId>profile-only</artifactId><version>1.0.0</version>
              </dependency></dependencies></profile></profiles>
            </project>"#,
        )
        .unwrap();
        let dependencies = pom.direct_dependencies();
        assert_eq!(dependencies.len(), 1);
        assert_eq!(dependencies[0].coordinate, "com.google.guava:guava");
        assert_eq!(dependencies[0].version.as_deref(), Some("33.0.0-jre"));
    }

    #[test]
    fn only_release_distribution_suffixes_project_to_semver() {
        assert_eq!(
            maven_release_version("33.0.0-jre").unwrap(),
            Version::new(33, 0, 0)
        );
        assert!(maven_release_version("33.0.0-rc1").is_none());
        assert!(maven_release_version("LATEST").is_none());
    }

    #[test]
    fn doctypes_and_unresolved_properties_fail_closed() {
        assert!(parse_pom("<!DOCTYPE project><project/>").is_err());
        assert!(expand_properties("${env.GUAVA_VERSION}", &BTreeMap::new()).is_none());
    }

    #[test]
    fn missing_out_of_range_invalid_and_ambiguous_dependencies_are_distinct() {
        let source = SemanticPackDependencySource {
            declared_path: "pom.xml".to_string(),
            content_digest: "sha256:test".to_string(),
        };
        let mut catalog = MavenCatalog::default();
        catalog.declarations.insert(
            "present:library".to_string(),
            vec![MavenDeclaration {
                version: Some("2.0.0".to_string()),
                source: source.clone(),
            }],
        );
        catalog.declarations.insert(
            "invalid:library".to_string(),
            vec![MavenDeclaration {
                version: Some("LATEST".to_string()),
                source: source.clone(),
            }],
        );
        catalog.declarations.insert(
            "ambiguous:library".to_string(),
            vec![
                MavenDeclaration {
                    version: Some("1.0.0".to_string()),
                    source: source.clone(),
                },
                MavenDeclaration {
                    version: Some("2.0.0".to_string()),
                    source,
                },
            ],
        );

        assert!(matches!(
            catalog.resolve("missing:library", ">=1.0.0"),
            MavenResolution::Missing
        ));
        assert!(matches!(
            catalog.resolve("present:library", ">=3.0.0"),
            MavenResolution::OutOfRange
        ));
        assert!(matches!(
            catalog.resolve("invalid:library", ">=1.0.0"),
            MavenResolution::InvalidVersion
        ));
        assert!(matches!(
            catalog.resolve("ambiguous:library", ">=1.0.0"),
            MavenResolution::AmbiguousVersion
        ));
    }
}

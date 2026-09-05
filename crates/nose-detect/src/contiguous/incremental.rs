use super::{detect_primitives, groups_from_primitives, k, kgrams, LocSeed, Stream};
use crate::cluster::UnionFind;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const STATE_SCHEMA: u32 = 3;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
struct StreamKey {
    digest: [u8; 32],
    occurrence: u32,
}

#[derive(Serialize, Deserialize)]
struct StoredLocSeed {
    stream: StreamKey,
    start_line: u32,
    end_line: u32,
    sem: usize,
    source_lines: (u32, u32),
}

#[derive(Serialize, Deserialize)]
struct StoredComponent {
    streams: Vec<StreamKey>,
    locs: Vec<StoredLocSeed>,
    pairs: Vec<(u32, u32)>,
}

type DetectionPrimitives = (Vec<LocSeed>, Vec<(usize, usize)>);

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct IncrementalContiguousState {
    schema: u32,
    streams: Vec<StreamKey>,
    components: Vec<StoredComponent>,
}

#[derive(Default)]
pub(crate) struct IncrementalContiguousStats {
    pub(crate) components_reused: usize,
    pub(crate) components_rebuilt: usize,
    pub(crate) streams_reused: usize,
    pub(crate) streams_rebuilt: usize,
}

pub(crate) fn detect_incremental(
    streams: &[Stream],
    min_tokens: usize,
    min_lines: u32,
    trace_accepted_coverage: bool,
    previous: Option<IncrementalContiguousState>,
) -> (
    Vec<crate::Group>,
    Vec<Vec<crate::AcceptedEdge>>,
    IncrementalContiguousState,
    IncrementalContiguousStats,
) {
    let window = k();
    let keys = stream_keys(streams);
    let previous = previous.filter(|state| state.schema == STATE_SCHEMA);
    if previous.as_ref().is_some_and(|state| state.streams == keys) {
        return reuse_unchanged(
            streams,
            keys,
            previous.expect("checked state"),
            trace_accepted_coverage,
        );
    }
    let components = stream_components(streams, window);
    let prior = previous
        .as_ref()
        .into_iter()
        .flat_map(|state| &state.components)
        .map(|component| (component.streams.as_slice(), component))
        .collect::<BTreeMap<_, _>>();
    let current_index = keys
        .iter()
        .copied()
        .enumerate()
        .map(|(index, key)| (key, index))
        .collect::<BTreeMap<_, _>>();

    let mut groups = Vec::new();
    let mut edges = Vec::new();
    let mut stored_components = Vec::with_capacity(components.len());
    let mut stats = IncrementalContiguousStats::default();
    for members in components {
        let member_keys = members.iter().map(|&index| keys[index]).collect::<Vec<_>>();
        let (locs, pairs, reused) = prior
            .get(member_keys.as_slice())
            .and_then(|component| restore_component(component, &current_index))
            .map_or_else(
                || {
                    let mut grams = (0..streams.len()).map(|_| Vec::new()).collect::<Vec<_>>();
                    for &member in &members {
                        grams[member] = kgrams(&streams[member].tags, window);
                    }
                    let (locs, pairs) =
                        detect_primitives(streams, &grams, &members, window, min_tokens, min_lines);
                    (locs, pairs, false)
                },
                |(locs, pairs)| (locs, pairs, true),
            );
        if reused {
            stats.components_reused += 1;
            stats.streams_reused += members.len();
        } else {
            stats.components_rebuilt += 1;
            stats.streams_rebuilt += members.len();
        }
        let stored = store_component(member_keys, &locs, &pairs, &keys);
        let (component_groups, component_edges) =
            groups_from_primitives(locs, pairs, streams, trace_accepted_coverage);
        groups.extend(component_groups);
        edges.extend(component_edges);
        stored_components.push(stored);
    }
    (
        groups,
        edges,
        IncrementalContiguousState {
            schema: STATE_SCHEMA,
            streams: keys,
            components: stored_components,
        },
        stats,
    )
}

fn reuse_unchanged(
    streams: &[Stream],
    keys: Vec<StreamKey>,
    state: IncrementalContiguousState,
    trace_accepted_coverage: bool,
) -> (
    Vec<crate::Group>,
    Vec<Vec<crate::AcceptedEdge>>,
    IncrementalContiguousState,
    IncrementalContiguousStats,
) {
    let current_index = keys
        .iter()
        .copied()
        .enumerate()
        .map(|(index, key)| (key, index))
        .collect::<BTreeMap<_, _>>();
    let mut groups = Vec::new();
    let mut edges = Vec::new();
    for component in &state.components {
        let (locs, pairs) = restore_component(component, &current_index)
            .expect("an unchanged stream set restores every component");
        let (component_groups, component_edges) =
            groups_from_primitives(locs, pairs, streams, trace_accepted_coverage);
        groups.extend(component_groups);
        edges.extend(component_edges);
    }
    let stats = IncrementalContiguousStats {
        components_reused: state.components.len(),
        streams_reused: streams.len(),
        ..IncrementalContiguousStats::default()
    };
    (groups, edges, state, stats)
}

#[derive(Clone, Copy)]
struct GramOwner {
    hash: u64,
    stream: u32,
    lang: nose_il::Lang,
}

fn stream_components(streams: &[Stream], window: usize) -> Vec<Vec<usize>> {
    let mut union = UnionFind::new(streams.len());
    let gram_count = streams
        .iter()
        .map(|stream| stream.tags.len().saturating_sub(window.saturating_sub(1)))
        .sum();
    let mut owners = Vec::with_capacity(gram_count);
    for (stream, item) in streams.iter().enumerate() {
        owners.extend(
            kgrams(&item.tags, window)
                .into_iter()
                .map(|hash| GramOwner {
                    hash,
                    stream: stream as u32,
                    lang: item.lang,
                }),
        );
    }
    owners.sort_unstable_by(|left, right| {
        left.hash
            .cmp(&right.hash)
            .then_with(|| left.lang.name().cmp(right.lang.name()))
            .then(left.stream.cmp(&right.stream))
    });
    let mut start = 0;
    while start < owners.len() {
        let first = owners[start];
        let mut end = start + 1;
        while end < owners.len() && owners[end].hash == first.hash && owners[end].lang == first.lang
        {
            end += 1;
        }
        for owner in &owners[start + 1..end] {
            if owner.stream != first.stream {
                union.union(first.stream as usize, owner.stream as usize);
            }
        }
        start = end;
    }
    let mut by_root = BTreeMap::<usize, Vec<usize>>::new();
    for index in 0..streams.len() {
        by_root.entry(union.find(index)).or_default().push(index);
    }
    let mut components = by_root.into_values().collect::<Vec<_>>();
    components.sort_unstable_by_key(|members| members[0]);
    components
}

fn stream_keys(streams: &[Stream]) -> Vec<StreamKey> {
    let digests = streams
        .iter()
        .map(|stream| {
            let bytes = rmp_serde::to_vec(stream).expect("Stream serialization cannot fail");
            let mut hasher = Sha256::new();
            hasher.update(b"nose.incremental-stream.v1\0");
            hasher.update((bytes.len() as u64).to_be_bytes());
            hasher.update(bytes);
            <[u8; 32]>::from(hasher.finalize())
        })
        .collect::<Vec<_>>();
    let mut occurrences = BTreeMap::<[u8; 32], u32>::new();
    digests
        .into_iter()
        .map(|digest| {
            let occurrence = occurrences.entry(digest).or_default();
            let key = StreamKey {
                digest,
                occurrence: *occurrence,
            };
            *occurrence += 1;
            key
        })
        .collect()
}

fn store_component(
    streams: Vec<StreamKey>,
    locs: &[LocSeed],
    pairs: &[(usize, usize)],
    keys: &[StreamKey],
) -> StoredComponent {
    StoredComponent {
        streams,
        locs: locs
            .iter()
            .map(|loc| StoredLocSeed {
                stream: keys[loc.stream],
                start_line: loc.start_line,
                end_line: loc.end_line,
                sem: loc.sem,
                source_lines: loc.source_lines,
            })
            .collect(),
        pairs: pairs
            .iter()
            .map(|&(left, right)| (left as u32, right as u32))
            .collect(),
    }
}

fn restore_component(
    component: &StoredComponent,
    current_index: &BTreeMap<StreamKey, usize>,
) -> Option<DetectionPrimitives> {
    let locs = component
        .locs
        .iter()
        .map(|loc| {
            Some(LocSeed {
                stream: *current_index.get(&loc.stream)?,
                start_line: loc.start_line,
                end_line: loc.end_line,
                sem: loc.sem,
                source_lines: loc.source_lines,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let pairs = component
        .pairs
        .iter()
        .map(|&(left, right)| Some((usize::try_from(left).ok()?, usize::try_from(right).ok()?)))
        .collect::<Option<Vec<_>>>()?;
    Some((locs, pairs))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(path: &str, tags: &[u64]) -> Stream {
        let mut value = Stream {
            root_is_module: false,
            source: None,
            path: path.to_owned(),
            lang: nose_il::Lang::Python,
            tags: tags.to_vec(),
            start: (1..=tags.len() as u32).collect(),
            end: (1..=tags.len() as u32).collect(),
            op: vec![true; tags.len()],
        };
        value.op[0] = true;
        value
    }

    #[test]
    fn unchanged_components_reuse_every_stream() {
        let shared = (100..130).collect::<Vec<_>>();
        let streams = vec![stream("a.py", &shared), stream("b.py", &shared)];
        let (_, _, state, cold) = detect_incremental(&streams, 10, 3, true, None);
        assert_eq!(cold.streams_rebuilt, 2);
        let (incremental, edges, _, warm) = detect_incremental(&streams, 10, 3, true, Some(state));
        let clean = super::super::detect(&streams, 10, 3, true);
        assert_eq!(warm.streams_reused, 2);
        assert_eq!(incremental.len(), clean.0.len());
        assert_eq!(edges.len(), clean.1.len());
    }

    #[test]
    fn leaf_edit_rebuilds_only_its_kgram_component() {
        let first_shared = (100..130).collect::<Vec<_>>();
        let second_shared = (1_000..1_030).collect::<Vec<_>>();
        let before = vec![
            stream("a.py", &first_shared),
            stream("b.py", &first_shared),
            stream("c.py", &second_shared),
            stream("d.py", &second_shared),
        ];
        let (_, _, state, cold) = detect_incremental(&before, 10, 3, true, None);
        assert_eq!(cold.streams_rebuilt, 4);

        let changed_tags = (2_000..2_030).collect::<Vec<_>>();
        let after = vec![
            stream("a.py", &first_shared),
            stream("b.py", &changed_tags),
            stream("c.py", &second_shared),
            stream("d.py", &second_shared),
        ];
        let (incremental, _, _, stats) = detect_incremental(&after, 10, 3, true, Some(state));
        let clean = super::super::detect(&after, 10, 3, true).0;
        assert_eq!(stats.streams_reused, 2);
        assert_eq!(stats.streams_rebuilt, 2);
        assert_eq!(group_sites(&incremental), group_sites(&clean));
    }

    fn group_sites(groups: &[crate::Group]) -> Vec<Vec<(String, u32, u32)>> {
        let mut groups = groups
            .iter()
            .map(|group| {
                let mut members = group
                    .members
                    .iter()
                    .map(|member| (member.file.clone(), member.start_line, member.end_line))
                    .collect::<Vec<_>>();
                members.sort();
                members
            })
            .collect::<Vec<_>>();
        groups.sort();
        groups
    }
}

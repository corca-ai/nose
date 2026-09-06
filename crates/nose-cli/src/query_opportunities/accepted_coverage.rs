use super::FileOpportunityBucket;
use rustc_hash::FxHashMap;
use std::cmp::Ordering;

pub(super) fn opportunity_root(primary_index: &[Option<usize>], mut index: usize) -> usize {
    while let Some(primary) = primary_index[index] {
        index = primary;
    }
    index
}

pub(super) fn direct_suppression_forest(
    family_count: usize,
    direct_pairs: &[(usize, usize)],
) -> (Vec<Option<usize>>, Vec<usize>) {
    fn find(parent: &mut [usize], mut index: usize) -> usize {
        while parent[index] != index {
            parent[index] = parent[parent[index]];
            index = parent[index];
        }
        index
    }

    let mut union_parent: Vec<usize> = (0..family_count).collect();
    let mut adjacent = vec![Vec::new(); family_count];
    for &(left, right) in direct_pairs {
        let left_root = find(&mut union_parent, left);
        let right_root = find(&mut union_parent, right);
        let (root, child) = (left_root.min(right_root), left_root.max(right_root));
        union_parent[child] = root;
        adjacent[left].push(right);
        adjacent[right].push(left);
    }
    for neighbors in &mut adjacent {
        neighbors.sort_unstable();
        neighbors.dedup();
    }
    let mut roots = Vec::new();
    for index in 0..family_count {
        if find(&mut union_parent, index) == index {
            roots.push(index);
        }
    }
    let mut direct_parent = vec![None; family_count];
    let mut visited = vec![false; family_count];
    for &root in &roots {
        let mut queue = std::collections::VecDeque::from([root]);
        visited[root] = true;
        while let Some(current) = queue.pop_front() {
            for &neighbor in &adjacent[current] {
                if visited[neighbor] {
                    continue;
                }
                visited[neighbor] = true;
                direct_parent[neighbor] = Some(current);
                queue.push_back(neighbor);
            }
        }
    }
    (direct_parent, roots)
}

pub(super) fn accepted_obligations_covered(
    primary: &nose_detect::RefactorFamily,
    slice: &nose_detect::RefactorFamily,
) -> bool {
    edges_covered_by_family(primary, &slice.locations, &slice.direct_edges)
        && slice.accepted_coverage.iter().all(|obligation| {
            obligation.edges.iter().all(|edge| {
                let Some(left) = obligation.sites.get(edge.left as usize) else {
                    return false;
                };
                let Some(right) = obligation.sites.get(edge.right as usize) else {
                    return false;
                };
                primary
                    .locations
                    .iter()
                    .any(|loc| site_is_covered(loc, left))
                    && primary
                        .locations
                        .iter()
                        .any(|loc| site_is_covered(loc, right))
            })
        })
}

fn edges_covered_by_family(
    primary: &nose_detect::RefactorFamily,
    sites: &[nose_detect::Loc],
    edges: &nose_detect::AcceptedEdges,
) -> bool {
    edges.iter().all(|edge| {
        let Some(left) = sites.get(edge.left as usize) else {
            return false;
        };
        let Some(right) = sites.get(edge.right as usize) else {
            return false;
        };
        primary
            .locations
            .iter()
            .any(|loc| site_is_covered(loc, left))
            && primary
                .locations
                .iter()
                .any(|loc| site_is_covered(loc, right))
    })
}

pub(super) fn accepted_edges_covered_by_roots(
    carrier: &nose_detect::RefactorFamily,
    coverage_roots: &[bool],
    by_file: &FxHashMap<&str, FileOpportunityBucket>,
) -> bool {
    edges_covered_by_roots(
        &carrier.locations,
        &carrier.direct_edges,
        coverage_roots,
        by_file,
    ) && carrier.accepted_coverage.iter().all(|obligation| {
        let roots_by_site = roots_by_site(&obligation.sites, coverage_roots, by_file);
        obligation.edges.iter().all(|edge| {
            let Some(left_roots) = roots_by_site.get(edge.left as usize) else {
                return false;
            };
            let Some(right_roots) = roots_by_site.get(edge.right as usize) else {
                return false;
            };
            sorted_lists_intersect(left_roots, right_roots)
        })
    })
}

fn edges_covered_by_roots(
    sites: &[nose_detect::Loc],
    edges: &nose_detect::AcceptedEdges,
    coverage_roots: &[bool],
    by_file: &FxHashMap<&str, FileOpportunityBucket>,
) -> bool {
    let roots_by_site = roots_by_site(sites, coverage_roots, by_file);
    edges.iter().all(|edge| {
        let Some(left_roots) = roots_by_site.get(edge.left as usize) else {
            return false;
        };
        let Some(right_roots) = roots_by_site.get(edge.right as usize) else {
            return false;
        };
        sorted_lists_intersect(left_roots, right_roots)
    })
}

fn roots_by_site(
    sites: &[nose_detect::Loc],
    coverage_roots: &[bool],
    by_file: &FxHashMap<&str, FileOpportunityBucket>,
) -> Vec<Vec<usize>> {
    sites
        .iter()
        .map(|site| {
            let mut roots = Vec::new();
            let Some(bucket) = by_file.get(site.file.as_str()) else {
                return roots;
            };
            // `bucket.intervals` already contains every location in this file in family-rank
            // order. Query it directly instead of visiting each root and rescanning all of that
            // family's locations across every file. A family is appended once even when several
            // of its same-file members cover the site.
            for interval in &bucket.intervals {
                if !coverage_roots[interval.family]
                    || roots.last() == Some(&interval.family)
                    || !lines_cover_site(interval.start, interval.end, site)
                {
                    continue;
                }
                roots.push(interval.family);
            }
            roots
        })
        .collect()
}

fn lines_cover_site(start: u32, end: u32, site: &nose_detect::Loc) -> bool {
    let lo = start.max(site.start_line);
    let hi = end.min(site.end_line);
    if lo > hi {
        return false;
    }
    let overlap = hi - lo + 1;
    let site_len = site.end_line - site.start_line + 1;
    overlap * 2 >= site_len
}

fn sorted_lists_intersect(left: &[usize], right: &[usize]) -> bool {
    let (mut i, mut j) = (0, 0);
    while let (Some(&a), Some(&b)) = (left.get(i), right.get(j)) {
        match a.cmp(&b) {
            Ordering::Less => i += 1,
            Ordering::Greater => j += 1,
            Ordering::Equal => return true,
        }
    }
    false
}

fn site_is_covered(outer: &nose_detect::Loc, site: &nose_detect::Loc) -> bool {
    if outer.file != site.file {
        return false;
    }
    let lo = outer.start_line.max(site.start_line);
    let hi = outer.end_line.min(site.end_line);
    if lo > hi {
        return false;
    }
    let overlap = hi - lo + 1;
    let site_len = site.end_line - site.start_line + 1;
    overlap * 2 >= site_len
}

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
    slice.accepted_coverage.iter().all(|obligation| {
        obligation.edges.iter().all(|&(left, right)| {
            let Some(left) = obligation.sites.get(left as usize) else {
                return false;
            };
            let Some(right) = obligation.sites.get(right as usize) else {
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

pub(super) fn accepted_edges_covered_by_roots(
    carrier: &nose_detect::RefactorFamily,
    families: &[&nose_detect::RefactorFamily],
    coverage_roots: &[bool],
    by_file: &FxHashMap<&str, FileOpportunityBucket>,
) -> bool {
    carrier.accepted_coverage.iter().all(|obligation| {
        let roots_by_site: Vec<Vec<usize>> = obligation
            .sites
            .iter()
            .map(|site| {
                by_file
                    .get(site.file.as_str())
                    .into_iter()
                    .flat_map(|bucket| bucket.families.iter().copied())
                    .filter(|&family_index| {
                        coverage_roots[family_index]
                            && families[family_index]
                                .locations
                                .iter()
                                .any(|loc| site_is_covered(loc, site))
                    })
                    .collect()
            })
            .collect();
        obligation.edges.iter().all(|&(left, right)| {
            let Some(left_roots) = roots_by_site.get(left as usize) else {
                return false;
            };
            let Some(right_roots) = roots_by_site.get(right as usize) else {
                return false;
            };
            sorted_lists_intersect(left_roots, right_roots)
        })
    })
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

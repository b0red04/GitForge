use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use gitforge_graph::{CommitEntry, Graph};

fn generate_linear_commits(count: usize) -> Vec<CommitEntry> {
    let mut commits = Vec::with_capacity(count);
    for i in 0..count {
        let id = format!("{:040x}", i);
        let parents = if i + 1 < count {
            vec![format!("{:040x}", i + 1)]
        } else {
            vec![]
        };
        commits.push(CommitEntry::new(id, parents));
    }
    commits
}

fn generate_branchy_commits(count: usize) -> Vec<CommitEntry> {
    let mut commits = Vec::with_capacity(count);
    let branch_interval = 10;

    for i in 0..count {
        let id = format!("{:040x}", i);
        let parents = if i % branch_interval == 0 && i + branch_interval < count && i + 1 < count {
            vec![
                format!("{:040x}", i + 1),
                format!("{:040x}", i + branch_interval),
            ]
        } else if i + 1 < count {
            vec![format!("{:040x}", i + 1)]
        } else {
            vec![]
        };
        commits.push(CommitEntry::new(id, parents));
    }
    commits
}

fn bench_graph_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_build");

    for size in [100, 1_000, 10_000, 50_000] {
        let linear = generate_linear_commits(size);
        group.bench_with_input(BenchmarkId::new("linear", size), &linear, |b, commits| {
            b.iter(|| black_box(Graph::build(black_box(commits))));
        });

        let branchy = generate_branchy_commits(size);
        group.bench_with_input(BenchmarkId::new("branchy", size), &branchy, |b, commits| {
            b.iter(|| black_box(Graph::build(black_box(commits))));
        });
    }

    group.finish();
}

fn bench_graph_row_lookup(c: &mut Criterion) {
    let commits = generate_linear_commits(10_000);
    let graph = Graph::build(&commits);

    c.bench_function("row_for_commit_hit", |b| {
        let mid_id = &commits[5_000].id;
        b.iter(|| black_box(graph.row_for_commit(black_box(mid_id))));
    });

    c.bench_function("row_for_commit_miss", |b| {
        b.iter(|| black_box(graph.row_for_commit(black_box("nonexistent"))));
    });
}

fn naive_visible_line_indices(graph: &Graph, rows: std::ops::Range<usize>) -> Vec<usize> {
    graph
        .lines()
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            line.full_interval.start < rows.end && line.full_interval.end >= rows.start
        })
        .map(|(idx, _)| idx)
        .collect()
}

fn bench_visible_line_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("visible_line_lookup");
    let viewport_len = 40;

    for (shape, commits) in [
        ("linear", generate_linear_commits(50_000)),
        ("branchy", generate_branchy_commits(50_000)),
    ] {
        let graph = Graph::build(&commits);
        for (position, start) in [("top", 0), ("middle", 25_000), ("end", 49_960)] {
            let rows = start..start + viewport_len;
            group.bench_with_input(
                BenchmarkId::new(format!("{shape}_indexed"), position),
                &rows,
                |b, rows| {
                    b.iter(|| black_box(graph.visible_line_indices(black_box(rows.clone()))));
                },
            );
            group.bench_with_input(
                BenchmarkId::new(format!("{shape}_naive"), position),
                &rows,
                |b, rows| {
                    b.iter(|| {
                        black_box(naive_visible_line_indices(&graph, black_box(rows.clone())))
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_scroll_visible_line_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("scroll_visible_line_lookup");
    let viewport_len = 40;

    for (shape, commits) in [
        ("linear", generate_linear_commits(50_000)),
        ("branchy", generate_branchy_commits(50_000)),
    ] {
        let graph = Graph::build(&commits);
        let max_start = commits.len().saturating_sub(viewport_len);
        group.bench_function(shape, |b| {
            b.iter(|| {
                let mut total_visible_lines = 0usize;
                for start in 0..=max_start {
                    let visible =
                        graph.visible_line_indices(black_box(start..start + viewport_len));
                    total_visible_lines += visible.len();
                }
                black_box(total_visible_lines)
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_graph_build,
    bench_graph_row_lookup,
    bench_visible_line_lookup,
    bench_scroll_visible_line_lookup
);
criterion_main!(benches);

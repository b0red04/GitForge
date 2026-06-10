use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use gitforge_graph::{CommitEntry, Graph};

fn generate_linear_commits(count: usize) -> Vec<CommitEntry> {
    let mut commits = Vec::with_capacity(count);
    for i in (0..count).rev() {
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

    for i in (0..count).rev() {
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
            b.iter(|| Graph::build(black_box(commits)));
        });

        let branchy = generate_branchy_commits(size);
        group.bench_with_input(BenchmarkId::new("branchy", size), &branchy, |b, commits| {
            b.iter(|| Graph::build(black_box(commits)));
        });
    }

    group.finish();
}

fn bench_graph_row_lookup(c: &mut Criterion) {
    let commits = generate_linear_commits(10_000);
    let graph = Graph::build(&commits);

    c.bench_function("row_for_commit_hit", |b| {
        let mid_id = &commits[5_000].id;
        b.iter(|| graph.row_for_commit(black_box(mid_id)));
    });

    c.bench_function("row_for_commit_miss", |b| {
        b.iter(|| graph.row_for_commit(black_box("nonexistent")));
    });
}

criterion_group!(benches, bench_graph_build, bench_graph_row_lookup);
criterion_main!(benches);

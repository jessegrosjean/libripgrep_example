extern crate ignore;
extern crate num_cpus;
extern crate grep;
extern crate grep_regex;
extern crate grep_matcher;
extern crate grep_searcher;
extern crate crossbeam;

use std::error::Error;
use std::io;
use std::cmp;
use std::sync::{Arc};
use std::env;

use ignore::{
    DirEntry,
    WalkBuilder
};

use crossbeam::queue::MsQueue;

use grep::matcher::LineTerminator;

use grep_regex::RegexMatcher;

use grep_matcher:: {
    Matcher
};

use grep_searcher:: {
    Sink,
    Searcher,
    SinkMatch,
    SearcherBuilder,
    MmapChoice,
};

pub struct FileMatch {
    entry: DirEntry,
    matche_count: u64
}

pub fn scan_path(pattern: &str, path: &str) -> Result<(Vec<FileMatch>), Box<Error>> {
    let matcher = RegexMatcher::new(&pattern)?;
    let parallel_walker = WalkBuilder::new(path)
        .threads(cmp::min(12, num_cpus::get()))
        .build_parallel();

    let queue: Arc<MsQueue<Option<FileMatch>>> = Arc::new(MsQueue::new());

    parallel_walker.run(|| {
        let push_queue = queue.clone();
        let matcher = matcher.clone();

        let mut searcher = SearcherBuilder::new()
            .line_terminator(LineTerminator::byte(b'\n'))
            .memory_map( unsafe { MmapChoice::auto() })
            .line_number(false)
            .multi_line(false)
            .build();

        return Box::new(move |entry| {
            if let Ok(entry) = entry {                
                let mut matche_count = 0;
                let _ = searcher.search_path(&matcher, entry.path(), MatchesSink(|_searcher, sink_match| {
                    matcher.find_iter(sink_match.bytes(), |_m| {
                        matche_count += 1;
                        true
                    }).unwrap();
                    Ok(true)
                }));

                if matche_count > 0 {
                    let file_match = FileMatch { entry: entry, matche_count: matche_count };
                    push_queue.push(Some(file_match));
                }
            }
            ignore::WalkState::Continue
        })
    });

    queue.push(None);

    let mut matched_files = Vec::new();    
    while let Some(file_match) = queue.pop() {
        matched_files.push(file_match);
    }
    Ok(matched_files)
}

pub struct MatchesSink<F>(pub F) where F: FnMut(&Searcher, &SinkMatch) -> Result<bool, io::Error>;

impl<F> Sink for MatchesSink<F> where F: FnMut(&Searcher, &SinkMatch) -> Result<bool, io::Error> {
    type Error = io::Error;
    fn matched(&mut self, searcher: &Searcher, sink_match: &SinkMatch) -> Result<bool, io::Error> {
        (self.0)(searcher, sink_match)
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let needel = &args[1];
    let haystack = &args[2];    

    if let Ok(results) = scan_path(needel, haystack) {
        for each in results {
            println!("{:?}:{}", each.entry.path(), each.matche_count);
        }
    }
}
use iced::futures::Stream;
use ignore::WalkBuilder;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;

pub fn normalized_path(path: &Path) -> PathBuf {
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_relative()
            && let Ok(cwd) = std::env::current_dir()
        {
            cwd.join(path)
        } else {
            path.to_path_buf()
        }
    });
    let s = abs.to_string_lossy();
    let stripped = if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        s.into_owned()
    };

    let mut normalized = PathBuf::new();
    for comp in Path::new(&stripped).components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }
    normalized
}

pub fn paths_equal(a: &Path, b: &Path) -> bool {
    let na = normalized_path(a);
    let nb = normalized_path(b);
    #[cfg(windows)]
    {
        na.to_string_lossy()
            .eq_ignore_ascii_case(&nb.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        na == nb
    }
}

pub struct StopFileWatcher {
    _watcher: RecommendedWatcher,
}

pub fn file_event_stream(paths: Vec<PathBuf>) -> impl Stream<Item = Vec<PathBuf>> {
    let (tx, mut rx) = mpsc::unbounded_channel::<PathBuf>();

    let watcher_res = notify::recommended_watcher(move |res: notify::Result<Event>| {
        if let Ok(event) = res {
            for path in event.paths {
                let _ = tx.send(path);
            }
        }
    });

    let guard = watcher_res.ok().map(|mut watcher| {
        for path in &paths {
            let mode = if path.is_dir() {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };
            let _ = watcher.watch(path, mode);
        }
        StopFileWatcher { _watcher: watcher }
    });

    iced::stream::channel(32, move |mut output: iced::futures::channel::mpsc::Sender<Vec<PathBuf>>| async move {
        let _guard = guard;
        let mut pending: HashSet<PathBuf> = HashSet::new();

        loop {
            let Some(first) = rx.recv().await else {
                break;
            };
            pending.insert(normalized_path(&first));

            let deadline = tokio::time::Instant::now() + Duration::from_millis(75);
            loop {
                match tokio::time::timeout_at(deadline, rx.recv()).await {
                    Ok(Some(path)) => {
                        pending.insert(normalized_path(&path));
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }

            if !pending.is_empty() {
                let batch: Vec<PathBuf> = pending.drain().collect();
                if output.try_send(batch).is_err() {
                    break;
                }
            }
        }
    })
}

pub fn index_workspace_files(root: PathBuf) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let walker = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if path.is_file() {
            files.push(path.to_path_buf());
        }
    }
    files
}

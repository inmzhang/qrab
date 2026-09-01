use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use miette::{Diagnostic, MietteError, MietteSpanContents, SourceCode, SourceSpan, SpanContents};
use thiserror::Error;

/// Expanded source text plus original file/line mappings.
#[derive(Debug, Clone)]
pub struct LoadedSource {
    // ponytail: original text duplicates expanded text for source-aware diagnostics;
    // use a source map if large imported modules make the memory cost measurable.
    text: String,
    line_starts: Vec<usize>,
    origins: Vec<(usize, usize)>,
    sources: Vec<SourceFile>,
}

#[derive(Debug, Clone)]
struct SourceFile {
    path: PathBuf,
    text: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    fn new(path: PathBuf, text: String) -> Self {
        let line_starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(offset, _)| offset + 1))
            .collect();
        Self {
            path,
            text,
            line_starts,
        }
    }
}

impl LoadedSource {
    /// Returns the expanded source accepted by [`crate::parse`] or [`crate::compile`].
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Maps a one-based expanded line to its original path and line number.
    pub fn origin(&self, expanded_line: usize) -> Option<(&Path, usize)> {
        self.origins
            .get(expanded_line.checked_sub(1)?)
            .and_then(|(source, line)| Some((self.sources.get(*source)?.path.as_path(), *line)))
    }
}

impl SourceCode for LoadedSource {
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn SpanContents<'a> + 'a>, MietteError> {
        let line = self
            .line_starts
            .partition_point(|start| *start <= span.offset())
            .checked_sub(1)
            .ok_or(MietteError::OutOfBounds)?;
        let expanded_start = self.line_starts[line];
        let (source, source_line) = *self.origins.get(line).ok_or(MietteError::OutOfBounds)?;
        let source = self.sources.get(source).ok_or(MietteError::OutOfBounds)?;
        let source_line_start = *source
            .line_starts
            .get(source_line - 1)
            .ok_or(MietteError::OutOfBounds)?;
        let local_offset = source_line_start
            .checked_add(span.offset() - expanded_start)
            .ok_or(MietteError::OutOfBounds)?;
        let local_span = (local_offset, span.len()).into();
        let mut lines_before = context_lines_before;
        let mut lines_after = context_lines_after;
        let (contents, context_offset) = loop {
            let contents = source
                .text
                .read_span(&local_span, lines_before, lines_after)?;
            let prefix = local_offset - contents.span().offset();
            if prefix > span.offset() {
                lines_before = lines_before
                    .checked_sub(1)
                    .ok_or(MietteError::OutOfBounds)?;
                continue;
            }
            let context_offset = span.offset() - prefix;
            if context_offset
                .checked_add(contents.span().len())
                .is_none_or(|end| end > self.text.len())
            {
                lines_after = lines_after.checked_sub(1).ok_or(MietteError::OutOfBounds)?;
                continue;
            }
            break (contents, context_offset);
        };
        let column = if contents.span().offset() == local_offset {
            source.text[source_line_start..local_offset].chars().count()
        } else {
            contents.column()
        };
        let contents = MietteSpanContents::new_named(
            source.path.display().to_string(),
            contents.data(),
            (context_offset, contents.span().len()).into(),
            contents.line(),
            column,
            contents.line_count(),
        )
        .with_language("qrab");
        Ok(Box::new(contents))
    }
}

/// An I/O, syntax, or cycle error encountered while expanding imports.
#[derive(Debug, Clone, PartialEq, Eq, Diagnostic, Error)]
#[error("{message}")]
pub struct LoadError {
    message: String,
}

impl LoadError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Loads a `.qrab` file and recursively expands file-scope relative imports.
///
/// Each canonical path is loaded once, and import cycles are rejected.
pub fn load_source(path: impl AsRef<Path>) -> Result<LoadedSource, LoadError> {
    let mut loaded = LoadedSource {
        text: String::new(),
        line_starts: Vec::new(),
        origins: Vec::new(),
        sources: Vec::new(),
    };
    load_file(
        path.as_ref(),
        &mut Vec::new(),
        &mut HashSet::new(),
        &mut loaded,
    )?;
    Ok(loaded)
}

fn load_file(
    path: &Path,
    stack: &mut Vec<PathBuf>,
    visited: &mut HashSet<PathBuf>,
    loaded: &mut LoadedSource,
) -> Result<(), LoadError> {
    let path = path
        .canonicalize()
        .map_err(|error| LoadError::new(format!("cannot read {}: {error}", path.display())))?;
    if let Some(position) = stack.iter().position(|entry| entry == &path) {
        let mut cycle = stack[position..]
            .iter()
            .map(|entry| entry.display().to_string())
            .collect::<Vec<_>>();
        cycle.push(path.display().to_string());
        return Err(LoadError::new(format!(
            "import cycle: {}",
            cycle.join(" -> ")
        )));
    }
    if visited.contains(&path) {
        return Ok(());
    }

    let source = fs::read_to_string(&path)
        .map_err(|error| LoadError::new(format!("cannot read {}: {error}", path.display())))?;
    let source_index = loaded.sources.len();
    loaded
        .sources
        .push(SourceFile::new(path.clone(), source.clone()));
    stack.push(path.clone());
    let mut depth = 0_usize;
    for (line_index, line) in source.lines().enumerate() {
        let import = import_path(line).map_err(|message| {
            LoadError::new(format!("{}:{}: {message}", path.display(), line_index + 1))
        })?;
        if let Some(import) = import {
            if depth != 0 {
                return Err(LoadError::new(format!(
                    "{}:{}: imports must be at file scope",
                    path.display(),
                    line_index + 1
                )));
            }
            let import = path.parent().unwrap_or(Path::new(".")).join(import);
            load_file(&import, stack, visited, loaded)?;
            continue;
        }

        loaded.line_starts.push(loaded.text.len());
        loaded.text.push_str(line);
        loaded.text.push('\n');
        loaded.origins.push((source_index, line_index + 1));
        let (opens, closes) = brace_counts(line);
        depth = depth.saturating_add(opens).saturating_sub(closes);
    }
    stack.pop();
    visited.insert(path);
    Ok(())
}

fn import_path(line: &str) -> Result<Option<&str>, &'static str> {
    let code = code_before_comment(line).trim();
    let Some(rest) = code.strip_prefix("import") else {
        return Ok(None);
    };
    if !rest.starts_with(char::is_whitespace) {
        return Ok(None);
    }
    let rest = rest.trim().strip_suffix(';').unwrap_or(rest.trim()).trim();
    let Some(path) = rest
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        return Err("import must be `import \"relative/path.qrab\"`");
    };
    if path.is_empty() {
        return Err("import path cannot be empty");
    }
    Ok(Some(path))
}

fn code_before_comment(line: &str) -> &str {
    let mut quoted = false;
    let mut escaped = false;
    let bytes = line.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if escaped {
            escaped = false;
        } else if *byte == b'\\' && quoted {
            escaped = true;
        } else if *byte == b'"' {
            quoted = !quoted;
        } else if !quoted && *byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            return &line[..index];
        }
    }
    line
}

fn brace_counts(line: &str) -> (usize, usize) {
    let mut quoted = false;
    let mut escaped = false;
    let mut counts = (0, 0);
    let code = code_before_comment(line);
    for character in code.chars() {
        if escaped {
            escaped = false;
        } else if character == '\\' && quoted {
            escaped = true;
        } else if character == '"' {
            quoted = !quoted;
        } else if !quoted && character == '{' {
            counts.0 += 1;
        } else if !quoted && character == '}' {
            counts.1 += 1;
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::parse;

    static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn loads_relative_modules_once_and_reports_cycles() {
        let directory = std::env::temp_dir().join(format!(
            "qrab-loader-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).expect("create loader test directory");
        fs::write(
            directory.join("gates.qrab"),
            "fn entangle(a, b) { h a; x b if a }\n",
        )
        .expect("write module");
        fs::write(
            directory.join("main.qrab"),
            "import \"gates.qrab\"\nimport \"gates.qrab\"\ncircuit main { qubit q[2]; entangle(q[0], q[1]) }\n",
        )
        .expect("write root source");

        let loaded = load_source(directory.join("main.qrab")).expect("load imports");
        let circuit = parse(loaded.as_str()).expect("parse expanded source");
        assert_eq!(circuit.operations.len(), 2);
        assert_eq!(loaded.origin(2).map(|(_, line)| line), Some(3));

        fs::write(directory.join("a.qrab"), "import \"b.qrab\"\n").expect("write cycle a");
        fs::write(directory.join("b.qrab"), "import \"a.qrab\"\n").expect("write cycle b");
        assert!(
            load_source(directory.join("a.qrab"))
                .expect_err("cycle must fail")
                .to_string()
                .contains("import cycle")
        );
        fs::write(
            directory.join("nested.qrab"),
            "circuit nested {\n  import \"gates.qrab\"\n  qubit q\n}\n",
        )
        .expect("write nested import");
        assert!(
            load_source(directory.join("nested.qrab"))
                .expect_err("nested import must fail in the loader")
                .to_string()
                .contains("imports must be at file scope")
        );

        fs::write(directory.join("empty.qrab"), "").expect("write empty module");
        fs::write(directory.join("context.qrab"), "x\nimport \"empty.qrab\"\n")
            .expect("write context source");
        let loaded = load_source(directory.join("context.qrab")).expect("load context source");
        let contents = loaded
            .read_span(&(0, 1).into(), 0, 2)
            .expect("read bounded context");
        assert!(contents.span().offset() + contents.span().len() <= loaded.as_str().len());
        fs::remove_dir_all(directory).expect("remove loader test directory");
    }
}

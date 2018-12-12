use std::sync::Arc;
use std::path::PathBuf;
use crate::fuzzy::FuzzyVec;
use crate::context_parser::InputContext;

pub struct CompletionSelector {
    /// Candidate completion entries.
    entries: Vec<Arc<String>>,
    // The currently selected entry.
    selected_index: usize,
    // The number of completion lines in the prompt.
    display_lines: usize,
    // The beginning of entries to be displayed.
    display_index: usize,
}

impl CompletionSelector {
    pub fn new(entries: Vec<Arc<String>>) -> CompletionSelector {
        const COMPLETION_LINES: usize = 7;

        CompletionSelector {
            entries,
            selected_index: 0,
            display_lines: COMPLETION_LINES,
            display_index: 0,
        }
    }

    /// Move to the next/previous entry.
    pub fn move_cursor(&mut self, offset: isize) {
        // FIXME: I think there's more sane way to handle a overflow.`
        let mut old_selected_index = self.selected_index as isize;
        old_selected_index += offset;

        let entries_len = self.len() as isize;
        if entries_len > 0 && old_selected_index > entries_len - 1 {
            old_selected_index = entries_len - 1;
        }

        if old_selected_index < 0 {
            old_selected_index = 0;
        }

        self.selected_index = old_selected_index as usize;

        if self.selected_index >= self.display_index + self.display_lines {
            self.display_index = self.selected_index - self.display_lines + 1;
        }

        if self.selected_index < self.display_index {
            self.display_index = self.selected_index;
        }

        trace!(
            "move_cursor: offset={}, index={}",
            offset,
            self.selected_index
        );
    }

    #[inline(always)]
    pub fn entries(&self) -> Vec<Arc<String>> {
        self.entries.clone()
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline(always)]
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    #[inline(always)]
    pub fn get(&self, index: usize) -> Option<Arc<String>> {
        self.entries.get(index).cloned()
    }

    #[inline(always)]
    pub fn display_lines(&self) -> usize {
        self.display_lines
    }

    #[inline(always)]
    pub fn display_index(&self) -> usize {
        self.display_index
    }

    pub fn select_and_update_input_and_cursor(&self, input_ctx: &InputContext, user_input: &mut String, user_cursor: &mut usize) {
        if let Some(selected) = self.get(self.selected_index()) {
            let prefix = user_input
                .get(..(input_ctx.current_word_offset))
                .unwrap_or("")
                .to_string();
            let suffix_offset = input_ctx.current_word_offset
                + input_ctx.current_word_len;
            let suffix =
                &user_input.get((suffix_offset)..).unwrap_or("").to_string();
            *user_input = format!("{}{}{}", prefix, selected, suffix);
            *user_cursor = input_ctx.current_word_offset + selected.len();
        }
    }
}

/// `compgen(1)`.
pub struct CompGen {
    entries: Vec<Arc<String>>,
    query: Option<String>,
    /// `-A command`
    include_commands: bool,
    /// `-A file`
    include_files: bool,
    /// `-A directory`
    include_dirs: bool,
}

impl CompGen {
    #[inline]
    pub fn new() -> CompGen {
        CompGen {
            entries: Vec::new(),
            query: None,
            include_commands: false,
            include_files: false,
            include_dirs: false,
        }
    }

    /// -A command / -c
    #[inline]
    pub fn include_commands<'a>(&'a mut self, enable: bool) -> &'a mut CompGen {
        self.include_commands = enable;
        self
    }

    /// -A file / -f
    #[inline]
    pub fn include_files<'a>(&'a mut self, enable: bool) -> &'a mut CompGen {
        self.include_files = enable;
        self
    }

    /// -A directory / -d
    #[inline]
    pub fn include_dirs<'a>(&'a mut self, enable: bool) -> &'a mut CompGen {
        self.include_dirs = enable;
        self
    }

    /// -W
    #[inline]
    pub fn wordlist<'a>(&'a mut self, wordlist: &str, ifs: &str) -> &'a mut CompGen {
        self.entries = wordlist
            .split(|c| ifs.contains(c))
            .map(|elem| Arc::new(elem.to_owned()))
            .collect();

        self
    }

    #[inline]
    pub fn entries<'a>(&'a mut self, entries: Vec<Arc<String>>) -> &'a mut CompGen {
        self.entries = entries;
        self
    }

    #[inline]
    pub fn filter_by<'a>(&'a mut self, query: &str) -> &'a mut CompGen {
        self.query = Some(query.to_owned());
        self
    }

    #[inline]
    pub fn generate(self) -> Vec<Arc<String>> {
        let results = match self.query {
            Some(query) => FuzzyVec::from_vec(self.entries).search(&query),
            None => self.entries,
        };

        results
    }
}

pub fn path_completion(ctx: &InputContext) -> Vec<Arc<String>> {
    let given_dir = ctx.current_word().map(|s| (&*s).clone());
    trace!("path_completion: current='{:?}', dir='{:?}'", ctx.current_word(), given_dir);
    let dirent = match &given_dir {
        Some(given_dir) if given_dir.ends_with('/') => {
            std::fs::read_dir(given_dir)
        },
        Some(given_dir) if given_dir.contains('/') => {
            // Remove the last part: `/Users/chandler/Docum' -> `/users/chandler'
            std::fs::read_dir(PathBuf::from(given_dir.clone()).parent().unwrap())
        },
        _ => {
            std::fs::read_dir(".")
        }
    };

    let mut entries = Vec::new();
    if let Ok(dirent) = dirent {
        for entry in dirent {
            let mut path = entry
                .unwrap()
                .path();

            if path.starts_with("./") {
                path = path.strip_prefix("./").unwrap().to_path_buf();
            }

            entries.push(Arc::new(path.to_str().unwrap().to_owned()));
        }
    }

    let mut compgen = CompGen::new();
    compgen.entries(entries);
    if let Some(current_word) = ctx.current_word() {
        compgen.filter_by(current_word.as_str());
    }
    compgen.generate()
}

pub fn cmd_completion(ctx: &InputContext) -> Vec<Arc<String>> {
    match ctx.current_word() {
        Some(query) => crate::path::complete(&query),
        None => crate::path::complete(""),
    }
}

#[derive(Debug, Clone)]
pub struct CompSpec {
    func_name: Option<String>,
    /// `-o filenames`
    filenames_if_empty: bool,
    /// `-o dirnames`
    dirnames_if_empty: bool,
}

impl CompSpec {
    #[inline]
    pub fn func_name(&self) -> Option<&String> {
        self.func_name.as_ref()
    }
}

#[derive(Debug)]
pub struct CompSpecBuilder {
    func_name: Option<String>,
    filenames_if_empty: bool,
    dirnames_if_empty: bool,
}

impl CompSpecBuilder {
    pub fn new() -> CompSpecBuilder {
        CompSpecBuilder {
            func_name: None,
            filenames_if_empty: false,
            dirnames_if_empty: false,
        }
    }

    #[inline]
    pub fn func_name<'a>(&'a mut self, func_name: String) -> &'a mut CompSpecBuilder {
        self.func_name = Some(func_name);
        self
    }

    /// -o dirnames
    #[inline]
    pub fn dirnames_if_empty<'a>(&'a mut self, enable: bool) -> &'a mut CompSpecBuilder {
        self.dirnames_if_empty = enable;
        self
    }

    /// -o filenames
    #[inline]
    pub fn filenames_if_empty<'a>(&'a mut self, enable: bool) -> &'a mut CompSpecBuilder {
        self.filenames_if_empty = enable;
        self
    }

    #[inline]
    pub fn build(self) -> CompSpec {
        CompSpec {
            func_name: self.func_name,
            filenames_if_empty: self.filenames_if_empty,
            dirnames_if_empty: self.dirnames_if_empty,
        }
    }
}

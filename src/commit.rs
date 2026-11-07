use std::collections::HashSet;
use std::fmt::Write;
use std::string::ToString;

use anyhow::Result;
use colored::Colorize;
use emojis::get_by_shortcode;
use git_conventional::Commit as ConventionalCommit;
use jiff::fmt::friendly::SpanPrinter;
use jiff::tz::{Offset, TimeZone};
use jiff::{SpanRound, Timestamp, Unit, Zoned};

pub struct Commit {
    pub id: String,
    pub message: String,
    pub timestamp: Zoned,
    pub url: String,
}

impl Commit {
    #[must_use]
    pub fn id(&self) -> String {
        hyperlink(&format!("{}/commit/{}", self.url, self.id), &self.id)
    }

    pub fn last_n_commits(n: usize) -> Result<Vec<Commit>> {
        //
        let commits = git2::Repository::discover(std::env::current_dir()?)
            .and_then(|repo| {
                disable_precompose_unicode(&repo)?;
                let url = origin_fetch_url(&repo.config()?)?;

                let mut revwalk = repo.revwalk()?;

                // Sort::TIME makes libgit2 walk the whole history up front; the default walk is lazy and date-ordered.
                revwalk.push_head()?;

                Ok(revwalk
                    .filter_map(|oid_result| oid_result.ok().and_then(|oid| repo.find_commit(oid).ok()))
                    .take(n)
                    .map(|commit| Commit {
                        id: commit
                            .as_object()
                            .short_id()
                            .ok()
                            .and_then(|buf| buf.as_str().ok().map(ToString::to_string))
                            .unwrap_or_default(),
                        message: commit.message().unwrap_or_default().to_string(),
                        timestamp: zoned_from_time(&commit.time()),
                        url: url.clone(),
                    })
                    .collect())
            })
            .unwrap_or_default();

        Ok(commits)
    }

    fn format_emoji(type_str: &str, scope: Option<&str>, other: Option<&str>, breaking: bool) -> String {
        let mut emojis: HashSet<String> = HashSet::new();

        // Add breaking change emoji if needed
        if breaking {
            get_by_shortcode("boom").map(|g| emojis.insert(g.as_str().to_string()));
        }

        if let Some(emoji) = commit_emoji(type_str) {
            emojis.insert(emoji.to_string());
        }

        if let Some(scope_str) = scope {
            //
            // Try combined type-scope emoji
            if let Some(g) = get_by_shortcode(&format!("{type_str}-{scope_str}")) {
                emojis.insert(g.as_str().to_string());
                //
            } else if let Some(g) = commit_emoji(scope_str) {
                emojis.insert(g.to_string());
            }
        }

        // Add other emojis if present
        if let Some(other_str) = other {
            other_str.split(':').filter(|s| !s.is_empty()).for_each(|code| {
                //
                if let Some(g) = get_by_shortcode(code) {
                    emojis.insert(g.as_str().to_string());
                }
            });
        }

        emojis.into_iter().collect::<Vec<_>>().join(" ")
    }

    pub fn format(&self, now: &Zoned, printer: &SpanPrinter) -> Result<String> {
        //
        let text = &self.message;
        let mut formatted = text.clone();

        // Try to parse as a conventional commit
        if let Ok(cc) = ConventionalCommit::parse(text) {
            let type_str = cc.type_().to_string();
            let scope = cc.scope().map(|s| s.as_str());
            let breaking = cc.breaking();
            let description = cc.description();

            // Extract any existing emoji codes from the description
            let other = if description.contains(':') {
                Some(description)
            } else {
                None
            };

            let emoji = Self::format_emoji(&type_str, scope, other, breaking);

            if !emoji.is_empty() {
                let mut header = type_str;

                if let Some(scope_str) = scope {
                    write!(header, "({})", scope_str.bold())?;
                }

                if breaking {
                    header.push('!');
                }

                formatted = format!("{} {emoji} {description}", format!("{header}:").blue());
            }
        }

        // Emit a string in the form of: "(1 year, 4 months, 28 days, 18 hours ago)"
        let span = (now - &self.timestamp).round(
            SpanRound::new()
                .largest(Unit::Year)
                .smallest(Unit::Hour)
                .relative(&self.timestamp),
        )?;

        write!(formatted, " ({} ago)", printer.span_to_string(&span))?;

        Ok(formatted.trim().lines().next().unwrap_or_default().to_string())
    }
}

#[must_use]
fn commit_emoji(key: &str) -> Option<&'static str> {
    match key {
        "add" => Some("➕"),                                     // heavy_plus_sign
        "android" => Some("🤖"),                                 // robot
        "breaking" => Some("💥"),                                // boom
        "build" | "deps" | "dep" | "dependencies" => Some("📦"), // package
        "chore" | "maintenance" => Some("🔧"),                   // wrench
        "ci" | "cd" => Some("👷"),                               // construction_worker
        "config" => Some("⚙️"),                                  // gear
        "doc" | "docs" | "documentation" => Some("📚"),          // books
        "docker" => Some("🐳"),                                  // whale
        "feat" | "feature" => Some("✨"),                        // sparkles
        "fix" => Some("🐛"),                                     // bug
        "i18n" | "l10n" => Some("🌐"),                           // globe_with_meridians
        "kubernetes" | "k8s" => Some("☸️"),                      // wheel_of_dharma
        "lint" | "linter" => Some("🚨"),                         // rotating_light
        "linux" => Some("🐧"),                                   // penguin
        "macos" | "ios" => Some("🍎"),                           // apple
        "merge" => Some("🔀"),                                   // twisted_rightwards_arrows
        "perf" | "performance" => Some("⚡️"),                    // zap
        "ref" | "refactor" => Some("♻️"),                        // recycle
        "release" => Some("🚀"),                                 // rocket
        "remove" => Some("➖"),                                  // heavy_minus_sign
        "revert" => Some("⏪"),                                  // rewind
        "security" => Some("🔒"),                                // lock
        "style" => Some("🎨"),                                   // art
        "test" | "tests" => Some("✅"),                          // white_check_mark
        "typo" | "typos" => Some("✏️"),                          // pencil2
        "ui" | "ux" => Some("💄"),                               // lipstick
        "windows" => Some("🏁"),                                 // checkered_flag
        "wip" => Some("🚧"),                                     // construction
        _ => None,
    }
}

/// Fetch URL of `origin` with `url.<base>.insteadOf` applied, as `Remote::url()` reports it.
///
/// `find_remote` duplicates the whole config into a snapshot and parses refspecs, tags and prune
/// settings we never use; this reads only the two keys it needs.
fn origin_fetch_url(config: &git2::Config) -> Result<String, git2::Error> {
    let url = match config.get_string("remote.origin.url") {
        Ok(url) => url,
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            // A remote with only a pushurl still exists; libgit2 reports no fetch URL for it.
            config.get_string("remote.origin.pushurl")?;
            return Ok(String::new());
        }
        Err(e) => return Err(e),
    };

    // Longest matching prefix wins; the first one seen wins ties (libgit2 `apply_insteadof`).
    let mut best: Option<(usize, String)> = None;
    let mut entries = config.entries(Some(r"url\..*\.insteadof"))?;
    while let Some(entry) = entries.next() {
        let entry = entry?;
        let (Ok(name), Ok(prefix)) = (entry.name(), entry.value()) else {
            continue;
        };
        if url.starts_with(prefix) && prefix.len() > best.as_ref().map_or(0, |b| b.0) {
            best = Some((prefix.len(), name["url.".len()..name.len() - ".insteadof".len()].to_string()));
        }
    }

    Ok(match best {
        Some((len, base)) => format!("{base}{}", &url[len..]),
        None => url,
    })
}

unsafe extern "C" {
    // Not bound by libgit2-sys; exported by the vendored static libgit2.
    fn git_config_backend_from_string(
        out: *mut *mut libgit2_sys::git_config_backend,
        cfg: *const std::ffi::c_char,
        len: usize,
        opts: *mut std::ffi::c_void,
    ) -> std::ffi::c_int;
}

/// With `core.precomposeunicode` on (macOS default) every ref lookup calls `iconv_open`, which dlopens
/// libiconv modules (~1.4ms); our only ref name is the ASCII `HEAD`, so disable it in memory.
fn disable_precompose_unicode(repo: &git2::Repository) -> Result<(), git2::Error> {
    use git2::Binding;

    const CFG: &str = "[core]\n\tprecomposeunicode = false\n";
    let config = repo.config()?;
    let mut backend = std::ptr::null_mut();

    // SAFETY: `backend` is written by libgit2 on success and ownership passes to `config` in
    // `git_config_add_backend`; `CFG` outlives both calls and its length is passed explicitly.
    unsafe {
        let rc = git_config_backend_from_string(&raw mut backend, CFG.as_ptr().cast(), CFG.len(), std::ptr::null_mut());
        if rc < 0 {
            return Err(git2::Error::last_error(rc));
        }
        let rc = libgit2_sys::git_config_add_backend(config.raw(), backend, libgit2_sys::GIT_CONFIG_LEVEL_APP, repo.raw(), 0);
        if rc < 0 {
            return Err(git2::Error::last_error(rc));
        }
    }
    Ok(())
}

/// Emit an OSC-8 hyperlink escape sequence.
pub fn hyperlink(url: &str, text: &str) -> String {
    //
    format!("\x1B]8;;{url}\x1B\\{text}\x1B]8;;\x1B\\").cyan().to_string()
}

/// Turn a `git2::Time` into a `jiff::Zoned` timestamp, taking into account the TZ offset.
fn zoned_from_time(time: &git2::Time) -> Zoned {
    Timestamp::from_second(time.seconds())
        .unwrap()
        .to_zoned(TimeZone::fixed(
            Offset::from_seconds(time.offset_minutes() * 60).unwrap(),
        ))
}

#[cfg(test)]
mod tests {
    use super::origin_fetch_url;

    #[test]
    fn origin_fetch_url_matches_find_remote() {
        let cases = [
            "[remote \"origin\"]\n\turl = https://github.com/a/b\n",
            "[remote \"origin\"]\n\turl = \n",
            "[remote \"origin\"]\n\tpushurl = git@github.com:a/b\n",
            "[remote \"origin\"]\n\turl = gh:a/b\n[url \"https://github.com/\"]\n\tinsteadOf = gh:\n[url \"https://x/\"]\n\tinsteadOf = g\n[url \"https://y/\"]\n\tinsteadOf = gh:\n",
            "[remote \"origin\"]\n\turl = gh:a/b\n[url \"ssh://g/\"]\n\tpushInsteadOf = gh:\n",
            "[remote \"upstream\"]\n\turl = https://github.com/a/b\n",
        ];
        for (i, cfg) in cases.iter().enumerate() {
            let dir = std::env::temp_dir().join(format!("devmoji-log-test-{}-{i}", std::process::id()));
            let repo = git2::Repository::init(&dir).unwrap();
            let path = dir.join(".git/config");
            let mut contents = std::fs::read_to_string(&path).unwrap();
            contents.push_str(cfg);
            std::fs::write(&path, contents).unwrap();

            let expected = repo.find_remote("origin").map(|r| r.url().unwrap_or_default().to_string());
            let actual = origin_fetch_url(&repo.config().unwrap());
            assert_eq!(expected.ok(), actual.ok(), "case {i}: {cfg}");
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }
}

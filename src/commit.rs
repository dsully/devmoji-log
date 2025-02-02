use std::collections::HashSet;
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
        hyperlink(&format!("{}/commit/{}", &self.url, &self.id), &self.id)
    }

    pub fn last_n_commits(n: usize) -> Result<Vec<Commit>> {
        //
        let commits = git2::Repository::discover(std::env::current_dir()?)
            .and_then(|repo| {
                let remote = repo.find_remote("origin")?;
                let url = remote.url().unwrap_or_default().to_string();

                let mut revwalk = repo.revwalk()?;

                revwalk.push_head()?;
                revwalk.set_sorting(git2::Sort::TIME)?;

                Ok(revwalk
                    .filter_map(|oid_result| oid_result.ok().and_then(|oid| repo.find_commit(oid).ok()))
                    .take(n)
                    .map(|commit| Commit {
                        id: commit
                            .as_object()
                            .short_id()
                            .ok()
                            .and_then(|buf| buf.as_str().map(ToString::to_string))
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
                    header.push_str(&format!("({})", scope_str.bold()));
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

        formatted.push_str(&format!(" ({} ago)", &printer.span_to_string(&span)));

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

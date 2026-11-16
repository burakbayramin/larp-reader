//! `terraform plan`: every paragraph is the new content of an object the plan
//! creates or updates, filler is state being refreshed, and a new chapter is
//! another module's data sources being read.

use std::sync::OnceLock;

use super::base::{Line, Segment, chrome, chrome_owned, seg};
use super::codegen::Deck;
use super::filler::{Slot, expand, seq, words};
use super::palette::{BLUE, CYAN, DIM, FG, GREEN, YELLOW};
use super::{Mask, Rng, section};
use crate::book::{Paragraph, Pos};
use crate::text::{ragged, ragged_count};

/// Text lines sit inside a heredoc: `          + ` before each.
const INDENT: usize = 12;
/// Lines of a resource block around its text lines.
const FRAME: usize = 8;
const OBJECTS: &[&str] = &[
    "notes",
    "drafts",
    "archive",
    "manifests",
    "fixtures",
    "snapshots",
    "templates",
    "exports",
    "changelog",
    "runbooks",
    "seeds",
];

// -- filler ----------------------------------------------------------------------

fn vocab() -> Vec<(&'static str, Vec<String>)> {
    vec![
        (
            "t",
            words(&[
                "aws_iam_role",
                "aws_iam_policy",
                "aws_security_group",
                "aws_s3_bucket",
                "aws_lambda_function",
                "aws_cloudwatch_log_group",
                "aws_route53_record",
                "aws_sqs_queue",
                "aws_dynamodb_table",
                "aws_ecs_service",
                "aws_kms_key",
                "aws_ssm_parameter",
            ]),
        ),
        (
            "n",
            words(&[
                "main", "this", "worker", "api", "edge", "replica", "primary", "events", "audit",
                "default", "internal", "public",
            ]),
        ),
        (
            "m",
            words(&[
                "network", "storage", "queue", "auth", "billing", "search", "ingest", "observe",
            ]),
        ),
        ("id", seq(4_811, 97, 60)),
        ("s", seq(0, 1, 3)),
        (
            "num",
            (0..40)
                .map(|i| (1_042_771_903u64 + i * 61_000_211).to_string())
                .collect(),
        ),
    ]
}

const REFRESH: &[&str] = &[
    "{t}.{n}: Refreshing state... [id=prod-{n}-{id}]",
    "module.{m}.{t}.{n}: Refreshing state... [id=prod-{m}-{n}-{id}]",
    "module.{m}.data.aws_iam_policy_document.{n}: Reading...",
    "module.{m}.data.aws_iam_policy_document.{n}: Read complete after {s}s [id={num}]",
];

fn refresh_deck() -> &'static Deck {
    static D: OnceLock<Deck> = OnceLock::new();
    D.get_or_init(|| Deck::new(expand(REFRESH, &vocab(), 400), "terraform-refresh"))
}

/// A refresh line: the address in the text colour, what happens to it dim.
fn refresh_line(text: &str, width: usize) -> Line {
    let (addr, rest) = text.split_once(": ").unwrap_or((text, ""));
    chrome_owned(width, vec![seg(addr, FG), seg(format!(": {rest}"), DIM)])
}

#[derive(Default)]
pub struct TerraformMask {
    slot: Slot,
}

impl Mask for TerraformMask {
    fn name(&self) -> &'static str {
        "terraform"
    }

    fn header(&mut self, _rng: &mut Rng, width: usize) -> Vec<Line> {
        vec![
            chrome(width, &[("$ ", DIM), ("terraform plan -out=tfplan", FG)]),
            refresh_line(
                "data.aws_caller_identity.current: Read complete after 0s [id=318402776154]",
                width,
            ),
            chrome(width, &[]),
            chrome(
                width,
                &[(
                    "Terraform used the selected providers to generate the following execution",
                    FG,
                )],
            ),
            chrome(
                width,
                &[(
                    "plan. Resource actions are indicated with the following symbols:",
                    FG,
                )],
            ),
            chrome(width, &[("  + ", GREEN), ("create", FG)]),
            chrome(width, &[("  ~ ", YELLOW), ("update in-place", FG)]),
            chrome(width, &[]),
            chrome(
                width,
                &[("Terraform will perform the following actions:", FG)],
            ),
            chrome(width, &[]),
        ]
    }

    fn stream(&mut self) {
        self.slot.stream();
    }

    fn noise(&mut self, _rng: &mut Rng, width: usize) -> Line {
        refresh_line(refresh_deck().get(self.slot.take()), width)
    }

    fn line_count(&self, para: &Paragraph, width: usize) -> usize {
        ragged_count(&para.text, width.saturating_sub(INDENT).max(1)) + FRAME
    }

    fn render(
        &mut self,
        para: &Paragraph,
        pos: Pos,
        width: usize,
        _rng: &mut Rng,
        line_no_start: usize,
    ) -> Vec<Line> {
        self.slot.set(line_no_start);
        let name = OBJECTS[line_no_start % OBJECTS.len()];
        let addr = format!("aws_s3_object.{name}[\"{:04}\"]", line_no_start % 10_000);
        let create = line_no_start.is_multiple_of(3);
        let (mark, look, will) = if create {
            ("+", GREEN, "will be created")
        } else {
            ("~", YELLOW, "will be updated in-place")
        };
        let bold = FG.bold();
        let mut out = vec![
            chrome(width, &[(&format!("  # {addr} {will}"), bold)]),
            chrome(
                width,
                &[
                    (&format!("  {mark} "), look),
                    (&format!("resource \"aws_s3_object\" \"{name}\" {{"), FG),
                ],
            ),
        ];
        let id = format!("\"prod-{name}/{:04}.txt\"", line_no_start % 10_000);
        out.push(if create {
            chrome(
                width,
                &[
                    ("      + ", GREEN),
                    ("id      = ", FG),
                    ("(known after apply)", DIM),
                ],
            )
        } else {
            chrome(width, &[("        id      = ", FG), (&id, CYAN)])
        });
        out.push(chrome(
            width,
            &[(&format!("      {mark} "), look), ("content = <<-EOT", FG)],
        ));
        let lead: Segment = if create {
            seg(" ".repeat(INDENT), FG)
        } else {
            seg("          + ", GREEN)
        };
        for t in ragged(&para.text, width.saturating_sub(INDENT).max(1)) {
            out.push(Line::book(vec![lead.clone(), seg(t, FG.book())], pos));
        }
        out.push(chrome(width, &[("        EOT", FG)]));
        out.push(if create {
            chrome(
                width,
                &[
                    ("      + ", GREEN),
                    ("etag    = ", FG),
                    ("(known after apply)", DIM),
                ],
            )
        } else {
            chrome(width, &[("        # (4 unchanged attributes hidden)", DIM)])
        });
        out.push(chrome(width, &[("    }", FG)]));
        out.push(chrome(width, &[]));
        out
    }

    /// Another module's data sources being read.
    fn transition(&self, chapter: usize, width: usize, _line_no_start: usize) -> Vec<Line> {
        let name = section::name(chapter, 0);
        let addr = format!("module.{name}.data.aws_iam_policy_document.assume");
        let id = u64::from_str_radix(&section::hex(chapter, "tf-policy", 8), 16).unwrap_or(0);
        vec![
            refresh_line(&format!("{addr}: Reading..."), width),
            refresh_line(&format!("{addr}: Read complete after 0s [id={id}]"), width),
        ]
    }

    fn progress_label(&self, done: usize, total: usize, _width: usize) -> String {
        format!(
            "Plan: {} to add, {done} to change, 0 to destroy",
            total.saturating_sub(done)
        )
    }

    fn status_right(
        &self,
        _done: usize,
        _total: usize,
        _elapsed: std::time::Duration,
    ) -> Vec<Segment> {
        vec![
            seg("workspace: prod", BLUE),
            seg("  ·  ", DIM),
            seg("backend: s3", CYAN),
            seg("  ·  ", DIM),
            seg("lock: held", YELLOW),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn para() -> Paragraph {
        Paragraph::new("Sabah erkenden uyandı ve pencereyi açtı. ".repeat(4))
    }

    #[test]
    fn header_is_the_plan_preamble() {
        let text: Vec<String> = TerraformMask::default()
            .header(&mut Rng::seed_from_u64(1), 100)
            .iter()
            .map(Line::plain)
            .collect();
        assert_eq!(text[0], "$ terraform plan -out=tfplan");
        assert!(text.iter().any(|l| l == "  ~ update in-place"));
        assert!(
            text.iter()
                .any(|l| l == "Terraform will perform the following actions:")
        );
    }

    #[test]
    fn a_paragraph_is_a_heredoc_inside_a_resource_block() {
        for start in [1, 3] {
            let lines = TerraformMask::default().render(
                &para(),
                (0, 0),
                80,
                &mut Rng::seed_from_u64(1),
                start,
            );
            let text: Vec<String> = lines.iter().map(Line::plain).collect();
            assert!(text[0].starts_with("  # aws_s3_object."), "{text:?}");
            assert!(text[1].contains("resource \"aws_s3_object\""));
            assert!(text.iter().any(|l| l.ends_with("content = <<-EOT")));
            assert!(text.iter().any(|l| l == "        EOT"));
            assert_eq!(text[text.len() - 2], "    }");
            let book: Vec<&Line> = lines.iter().filter(|l| l.is_book).collect();
            assert!(book.len() > 1);
            assert!(book.iter().all(|l| l.segments[0].0.len() == INDENT));
        }
        let created =
            TerraformMask::default().render(&para(), (0, 0), 80, &mut Rng::seed_from_u64(1), 3);
        assert!(created[0].plain().ends_with("will be created"));
        let updated =
            TerraformMask::default().render(&para(), (0, 0), 80, &mut Rng::seed_from_u64(1), 1);
        assert!(updated[0].plain().ends_with("will be updated in-place"));
    }

    #[test]
    fn noise_is_state_being_refreshed_or_read() {
        let mut m = TerraformMask::default();
        let mut r = Rng::seed_from_u64(5);
        for _ in 0..50 {
            let p = m.noise(&mut r, 200).plain();
            assert!(
                p.contains(": Refreshing state... [id=")
                    || p.ends_with(": Reading...")
                    || p.contains(": Read complete after "),
                "{p}"
            );
        }
    }

    #[test]
    fn pool_is_large_and_clean() {
        let d = refresh_deck();
        assert!(d.len() >= 400, "{}", d.len());
        for i in 0..d.len() {
            let low = d.get(i).to_lowercase();
            assert!(!low.contains("book") && !low.contains("reader"), "{low}");
        }
    }
}

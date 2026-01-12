//! Spec template creation

use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

/// Create a new spec from template
pub fn create_spec_from_template(
    project_root: &Path,
    name: &str,
    type_: &str,
) -> Result<PathBuf> {
    // Determine output path
    let output_dir = project_root.join("daedalos-tools").join(name);
    let output_file = output_dir.join(format!("{}.spec.yaml", name));

    if output_file.exists() {
        bail!("Spec already exists: {}", output_file.display());
    }

    // Create directory
    std::fs::create_dir_all(&output_dir)?;

    // Get current date
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();

    // Generate template content based on type
    let content = match type_ {
        "tool" => generate_tool_template(name, &date),
        "library" => generate_library_template(name, &date),
        "service" => generate_service_template(name, &date),
        "doc" => generate_doc_template(name, &date),
        _ => generate_tool_template(name, &date),
    };

    // Write the file
    std::fs::write(&output_file, content)?;

    Ok(output_file)
}

fn generate_tool_template(name: &str, date: &str) -> String {
    format!(
        r#"name: {name}
version: 1.0
created: {date}

intent: |
  # WHY does this tool exist?
  # What problem does it solve?
  # What experience should users have?
  #
  # This is NOT what the tool does - it's WHY it matters.
  # Example: "Users should experiment fearlessly" not "Provides undo functionality"

constraints:
  # Hard requirements - things that MUST be true
  # - Must work offline
  # - Must be < 100ms for common operations
  # - No external dependencies beyond X

interface:
  commands:
    # command_name:
    #   args: "required <arg> [optional]"
    #   returns: "What the command outputs"
    #   example: "{name} command arg"

  exit_codes:
    0: success
    1: general error
    2: invalid arguments

examples:
  # Concrete usage scenarios - not just command examples
  - scenario: "User wants to..."
    context: "They are in situation X"
    action: "{name} command"
    result: "This happens"
    why_it_matters: "This improves their workflow because..."

decisions:
  # Choices you made and WHY
  # This prevents future developers from relitigating settled questions
  - choice: "We chose X over Y"
    why: |
      Explanation of reasoning
    alternatives:
      - option: "Alternative approach"
        rejected_because: "Why it didn't work"

anti_patterns:
  # What NOT to do - learned from experience
  - pattern: "Don't do X"
    why_bad: "Because it causes Y problem"

connects_to:
  # How this relates to other components
  # - component: other-tool
  #   relationship: "How they interact"

metrics:
  success_criteria:
    # How do we know this tool is working well?
    # - "< 100ms response time"
    # - "Users don't reach for alternative tools"
  failure_indicators:
    # Warning signs that something is wrong
    # - "Users complain about X"
    # - "Error rate > 1%"
"#,
        name = name,
        date = date
    )
}

fn generate_library_template(name: &str, date: &str) -> String {
    format!(
        r#"name: {name}
version: 1.0
created: {date}
type: library

intent: |
  # WHY does this library exist?
  # What abstraction does it provide?
  # What complexity does it hide?

constraints:
  # - No runtime dependencies beyond X
  # - Thread-safe
  # - Zero-copy where possible

interface:
  types:
    # Type definitions and their purpose

  functions:
    # Public functions and their contracts

examples:
  - scenario: "Using the library"
    context: "Developer needs to..."
    action: "Library call"
    result: "Expected outcome"

decisions:
  - choice: "API design choice"
    why: "Reasoning"

anti_patterns:
  - pattern: "Don't use this way"
    why_bad: "Because..."
"#,
        name = name,
        date = date
    )
}

fn generate_service_template(name: &str, date: &str) -> String {
    format!(
        r#"name: {name}
version: 1.0
created: {date}
type: service

intent: |
  # WHY does this service exist?
  # What capability does it provide?

constraints:
  # - Must start in < 1s
  # - Max memory: X
  # - Socket path: /run/daedalos/{name}.sock

interface:
  protocol: "Unix socket / HTTP / gRPC"
  endpoints:
    # Endpoint definitions

  events:
    # Events emitted

examples:
  - scenario: "Client interaction"
    context: "Client needs..."
    action: "API call"
    result: "Response"

decisions:
  - choice: "Architecture choice"
    why: "Reasoning"

anti_patterns:
  - pattern: "Don't do this"
    why_bad: "Because..."
"#,
        name = name,
        date = date
    )
}

fn generate_doc_template(name: &str, date: &str) -> String {
    format!(
        r#"name: {name}
version: 1.0
created: {date}
type: doc

intent: |
  # WHY does this documentation exist?
  # Who is the audience?
  # What should they learn?

constraints:
  # - Keep under X words
  # - Include examples for every concept
  # - Maintain parity with implementation

sections:
  # Major sections and their purpose

examples:
  - scenario: "Reader wants to..."
    context: "They are learning about..."
    action: "Read this section"
    result: "They understand..."

anti_patterns:
  - pattern: "Don't document implementation details"
    why_bad: "Creates staleness, not useful to users"
"#,
        name = name,
        date = date
    )
}

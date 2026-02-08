use serde_json::{json, Value};

fn strip_code_fences(s: &str) -> &str {
    let s = s.trim();
    let s = if s.starts_with("```json") {
        &s[7..]
    } else if s.starts_with("```") {
        &s[3..]
    } else {
        return s;
    };
    let s = s.trim_start_matches('\n');
    if let Some(end) = s.rfind("```") {
        s[..end].trim()
    } else {
        s.trim()
    }
}

pub fn generate_commit_info(
    diff: &str,
    new_branch: bool,
    api_key: &str,
    model: &str,
) -> Result<(String, Option<String>), String> {

    let branch_step = if new_branch {
        "\nStep 3 — Write the branch name as: <type>/<short-kebab-case-slug>\n\
              • Must use the SAME type prefix as the commit message\n\
              • 2–5 words in the slug, lowercase, separated by hyphens\n\
              • The slug should describe the change differently from the commit description\n"
    } else {
        ""
    };

    let json_schema = if new_branch {
        "{{\"commit_message\": \"...\", \"branch_name\": \"...\"}}"
    } else {
        "{{\"commit_message\": \"...\"}}"
    };

    let prompt = format!(
        "Analyze the following diff and generate a commit message{extra}.\n\n\
        Step 1 — Determine the change type. Pick exactly ONE from this list:\n\
          feat     — a new feature or capability (default)\n\
          fix      — a bug or behavior fix\n\
          refactor — pure code restructuring with **ZERO** behavior change\n\
          docs     — documentation only\n\
          style    — formatting, whitespace, linting (no logic change)\n\
          test     — adding or updating tests\n\
          chore    — build scripts, deps, config, tooling\n\
          ci       — CI/CD pipeline changes\n\n\
        Step 2 — Write the commit message as: <type>: <concise imperative description>\n\
          • Use the imperative mood (\"add\", not \"added\" or \"adds\")\n\
          • Keep it under 72 characters\n\
          • Do NOT capitalize the description\n\
          • No trailing period\n\
        {branch_step}\n\
        Respond ONLY with valid JSON, no markdown fences:\n\
        {json_schema}\n\n\
        Diff:\n{diff}",
        extra = if new_branch { " and branch name" } else { "" },
        branch_step = branch_step,
        json_schema = json_schema,
        diff = diff,
    );

    let body = json!({
        "model": model,
        "provider": {"order": ["groq"]},
        "messages": [
            {
                "role": "system",
                "content": "You are a git commit message generator that strictly follows Conventional Commits. You always classify changes into exactly one type (feat, fix, refactor, docs, style, test, chore, ci) before writing the message. Respond only with valid JSON, no markdown."
            },
            {
                "role": "user",
                "content": prompt
            }
        ],
        "temperature": 0.3
    });

    let resp = ureq::post("https://openrouter.ai/api/v1/chat/completions")
        .set("Authorization", &format!("Bearer {}", api_key))
        .set("Content-Type", "application/json")
        .send_json(&body)
        .map_err(|e| match e {
            ureq::Error::Status(code, response) => {
                let body = response.into_string().unwrap_or_default();
                format!("API error ({}): {}", code, body)
            }
            other => format!("Request failed: {}", other),
        })?;

    let json_resp: Value = resp
        .into_json()
        .map_err(|e| format!("Failed to parse API response: {}", e))?;

    let content = json_resp["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| "No content in API response".to_string())?;

    let cleaned = strip_code_fences(content);

    let parsed: Value = serde_json::from_str(cleaned)
        .map_err(|e| format!("Failed to parse AI JSON: {} -- raw: {}", e, cleaned))?;

    let commit_message = parsed["commit_message"]
        .as_str()
        .ok_or_else(|| "No commit_message in AI response".to_string())?
        .to_string();

    let branch_name = if new_branch {
        Some(
            parsed["branch_name"]
                .as_str()
                .ok_or_else(|| "No branch_name in AI response".to_string())?
                .to_string(),
        )
    } else {
        None
    };

    Ok((commit_message, branch_name))
}

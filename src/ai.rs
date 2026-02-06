use serde_json::{json, Value};
use std::env;

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
) -> Result<(String, Option<String>), String> {
    let api_key = env::var("OPENROUTER_API_KEY")
        .map_err(|_| "OPENROUTER_API_KEY environment variable not set".to_string())?;

    let prompt = if new_branch {
        format!(
            "Generate a git commit message and a branch name for the following diff.\n
            The commit message should be concise and descriptive, using conventional commits style (e.g. feat: ..., fix: ..., refactor: ...).\n
            The branch name should be short, lowercase, kebab-case (e.g. feat/add-auth, fix/login-bug).\n
            The branch name and commit message should convey the same change but be worded differently.\n
            Respond ONLY with valid JSON, no markdown fences: {{\"commit_message\": \"...\", \"branch_name\": \"...\"}}\n\n\
            Diff:\n{}",
            diff
        )
    } else {
        format!(
            "Generate a git commit message for the following diff.\n\
            The commit message should be concise and descriptive, using conventional commits style (e.g. feat: ..., fix: ..., refactor: ...).\n\
            Respond ONLY with valid JSON, no markdown fences: {{\"commit_message\": \"...\"}}\n\n\
            Diff:\n{}",
            diff
        )
    };

    let body = json!({
        "model": "openai/gpt-oss-20b",
        "provider": {"order": ["groq"]},
        "messages": [
            {
                "role": "system",
                "content": "You are a helpful assistant that generates git commit messages. Respond only with valid JSON, no markdown."
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

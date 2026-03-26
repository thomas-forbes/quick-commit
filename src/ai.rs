use serde_json::{json, Value};
use std::time::Instant;

use crate::config::SemanticType;

pub struct QueryStats {
    pub total_time_ms: Option<u128>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

pub struct GeneratedCommitInfo {
    pub commit_message: String,
    pub branch_name: Option<String>,
    pub stats: QueryStats,
}

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
    semantic_types: &[SemanticType],
) -> Result<GeneratedCommitInfo, String> {
    let total_start = Instant::now();
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

    let max_name_len = semantic_types
        .iter()
        .map(|t| t.name.len())
        .max()
        .unwrap_or(0);
    let types_list: String = semantic_types
        .iter()
        .map(|t| {
            format!(
                "  {:<width$} — {}",
                t.name,
                t.description,
                width = max_name_len
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let type_names: String = semantic_types
        .iter()
        .map(|t| t.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    let prompt = format!(
        "Analyze the following diff and generate a commit message{extra}.\n\n\
        Step 1 — Determine the change type. Pick exactly ONE from this list:\n\
        {types_list}\n\n\
        Step 2 — Write the commit message as: <type>: <concise imperative description>\n\
          • Use the imperative mood (\"add\", not \"added\" or \"adds\")\n\
          • Keep it generally under 72 characters\n\
          • Do NOT capitalize the description\n\
          • No trailing period\n\
        {branch_step}\n\
        Respond ONLY with valid JSON, no markdown fences:\n\
        {json_schema}\n\n\
        Diff:\n{diff}",
        extra = if new_branch { " and branch name" } else { "" },
        types_list = types_list,
        branch_step = branch_step,
        json_schema = json_schema,
        diff = diff,
    );

    let body = json!({
        "model": model,
        "reasoning": { "effort": "minimal" },
        "provider": {"order": ["groq"]},
        "messages": [
            {
                "role": "system",
                "content": format!("You are a git commit message generator that strictly follows Conventional Commits. You always classify changes into exactly one type ({}) before writing the message. Respond only with valid JSON, no markdown.", type_names)
            },
            {
                "role": "user",
                "content": prompt
            }
        ],
        "temperature": 0.3
    });

    let request_start = Instant::now();
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
    let local_total_time_ms = total_start.elapsed().as_millis();

    let json_resp: Value = resp
        .into_json()
        .map_err(|e| format!("Failed to parse API response: {}", e))?;

    let generation_id = json_resp["id"].as_str();

    let input_tokens = json_resp["usage"]["prompt_tokens"].as_u64();
    let output_tokens = json_resp["usage"]["completion_tokens"].as_u64();
    let total_time_ms = generation_id
        .and_then(|id| fetch_generation_timing(id, api_key).ok())
        .unwrap_or(Some(
            local_total_time_ms.max(request_start.elapsed().as_millis()),
        ));

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

    Ok(GeneratedCommitInfo {
        commit_message,
        branch_name,
        stats: QueryStats {
            total_time_ms,
            input_tokens,
            output_tokens,
        },
    })
}

fn fetch_generation_timing(id: &str, api_key: &str) -> Result<Option<u128>, String> {
    let resp = ureq::get("https://openrouter.ai/api/v1/generation")
        .set("Authorization", &format!("Bearer {}", api_key))
        .query("id", id)
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, response) => {
                let body = response.into_string().unwrap_or_default();
                format!("Generation API error ({}): {}", code, body)
            }
            other => format!("Generation request failed: {}", other),
        })?;

    let json_resp: Value = resp
        .into_json()
        .map_err(|e| format!("Failed to parse generation response: {}", e))?;

    let latency_ms = json_resp["data"]["latency"]
        .as_f64()
        .map(|ms| ms.round() as u128);
    let generation_time_ms = json_resp["data"]["generation_time"]
        .as_f64()
        .map(|ms| ms.round() as u128);
    Ok(match (latency_ms, generation_time_ms) {
        (Some(latency), Some(generation)) => Some(latency + generation),
        (Some(latency), None) => Some(latency),
        (None, Some(generation)) => Some(generation),
        (None, None) => None,
    })
}

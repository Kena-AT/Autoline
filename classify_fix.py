import re

# Read the file
with open('crates/autoline-core/src/classify.rs', 'r') as f:
    content = f.read()

# Find the KNOWN_BINARIES check block and insert AI tool detection before it
# We need to move the for_ai_tool check to before the KNOWN_BINARIES check

old_block = """    if KNOWN_BINARIES.contains(&first_token) {
        return InputKind::Command(HistoryKind::Command);
    }

    if first_token.ends_with(".exe")
        || first_token.ends_with(".bat")
        || first_token.ends_with(".cmd")
        || first_token.ends_with(".ps1")
        || first_token.ends_with(".sh")
    {
        return InputKind::Command(HistoryKind::Command);
    }

    if has_flag || contains_pipe_sem {
        return InputKind::Command(HistoryKind::Command);
    }

    let for_ai_tool = lower.starts_with("claude ")
        || lower.starts_with("gemini ")
        || lower.starts_with("ollama run ")
        || lower.starts_with("llm ");
    if for_ai_tool {
        return InputKind::Command(HistoryKind::Prompt);
    }"""

new_block = """    // Check for explicit AI tool prefixes FIRST (before known binaries),
    // so that "claude explain..." and "gemini write..." are recognized as prompts
    let for_ai_tool = lower.starts_with("claude ")
        || lower.starts_with("gemini ")
        || lower.starts_with("ollama run ")
        || lower.starts_with("llm ");
    if for_ai_tool {
        return InputKind::Command(HistoryKind::Prompt);
    }

    if KNOWN_BINARIES.contains(&first_token) {
        return InputKind::Command(HistoryKind::Command);
    }

    if first_token.ends_with(".exe")
        || first_token.ends_with(".bat")
        || first_token.ends_with(".cmd")
        || first_token.ends_with(".ps1")
        || first_token.ends_with(".sh")
    {
        return InputKind::Command(HistoryKind::Command);
    }

    if has_flag || contains_pipe_sem {
        return InputKind::Command(HistoryKind::Command);
    }"""

if old_block in content:
    content = content.replace(old_block, new_block)
    with open('crates/autoline-core/src/classify.rs', 'w') as f:
        f.write(content)
    print("Fixed classify.rs - moved AI tool detection before known binaries check")
else:
    print("Could not find the target block - file may have different structure")
    # Let's find where the AI tool check is
    idx = content.find("let for_ai_tool")
    if idx >= 0:
        print(f"Found 'let for_ai_tool' at position {idx}")
        # Show surrounding context
        start = max(0, idx - 50)
        end = min(len(content), idx + 200)
        print(content[start:end])

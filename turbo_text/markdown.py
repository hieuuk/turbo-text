import re


PATTERNS = [
    ("h1", re.compile(r"^(# .+)$", re.MULTILINE)),
    ("h2", re.compile(r"^(## .+)$", re.MULTILINE)),
    ("h3", re.compile(r"^(### .+)$", re.MULTILINE)),
    ("bold_asterisk", re.compile(r"(\*\*(?!\s)(.+?)(?<!\s)\*\*)")),
    ("bold_underscore", re.compile(r"(__(?!\s)(.+?)(?<!\s)__)")),
    ("italic_asterisk", re.compile(r"(?<!\*)(\*(?!\s|\*)(.+?)(?<!\s|\*)\*)(?!\*)")),
    ("italic_underscore", re.compile(r"(?<!_)(_(?!\s|_)(.+?)(?<!\s|_)_)(?!_)")),
    ("strikethrough", re.compile(r"(~~(?!\s)(.+?)(?<!\s)~~)")),
    ("inline_code", re.compile(r"(`[^`\n]+`)")),
    ("link", re.compile(r"(\[.+?\]\(.+?\))")),
    ("blockquote", re.compile(r"^(>.+)$", re.MULTILINE)),
    ("hr", re.compile(r"^([-*_]{3,})\s*$", re.MULTILINE)),
]


def configure_tags(text_widget, font_family, font_size):
    font_size = int(font_size)
    text_widget.tag_configure("h1", font=(font_family, int(font_size * 1.8), "bold"))
    text_widget.tag_configure("h2", font=(font_family, int(font_size * 1.5), "bold"))
    text_widget.tag_configure("h3", font=(font_family, int(font_size * 1.25), "bold"))
    text_widget.tag_configure("bold_asterisk", font=(font_family, font_size, "bold"))
    text_widget.tag_configure("bold_underscore", font=(font_family, font_size, "bold"))
    text_widget.tag_configure("italic_asterisk", font=(font_family, font_size, "italic"))
    text_widget.tag_configure("italic_underscore", font=(font_family, font_size, "italic"))
    text_widget.tag_configure("strikethrough", overstrike=True)
    text_widget.tag_configure("inline_code", font=("Consolas", font_size),
                               background="#2d2d2d", foreground="#ce9178")
    text_widget.tag_configure("inline_code_light", font=("Consolas", font_size),
                               background="#e8e8e8", foreground="#b5200d")
    text_widget.tag_configure("link", foreground="#569cd6", underline=True)
    text_widget.tag_configure("blockquote", foreground="#6a9955",
                               font=(font_family, font_size, "italic"))
    text_widget.tag_configure("hr", foreground="#555555")


ALL_TAGS = [name for name, _ in PATTERNS]


def apply_highlighting(text_widget, theme="dark"):
    content = text_widget.get("1.0", "end-1c")

    for tag in ALL_TAGS:
        text_widget.tag_remove(tag, "1.0", "end")
    text_widget.tag_remove("inline_code_light", "1.0", "end")

    for tag_name, pattern in PATTERNS:
        for match in pattern.finditer(content):
            start_idx = f"1.0+{match.start()}c"
            end_idx = f"1.0+{match.end()}c"

            actual_tag = tag_name
            if theme == "light":
                if tag_name == "inline_code":
                    actual_tag = "inline_code_light"

            text_widget.tag_add(actual_tag, start_idx, end_idx)

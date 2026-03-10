import json
import os

CONFIG_DIR = os.path.join(os.getenv("APPDATA", os.path.expanduser("~")), "TurboText")
CONFIG_FILE = os.path.join(CONFIG_DIR, "settings.json")

DEFAULTS = {
    "font_family": "Consolas",
    "font_size": 14,
    "theme": "dark",
    "bg_color": "",
    "fg_color": "",
    "last_directory": "",
    "show_line_numbers": True,
    "word_wrap": True,
}

THEMES = {
    "dark":            {"bg": "#1e1e1e", "fg": "#d4d4d4", "mode": "dark"},
    "light":           {"bg": "#ffffff", "fg": "#1e1e1e", "mode": "light"},
    "monokai":         {"bg": "#272822", "fg": "#f8f8f2", "mode": "dark"},
    "solarized_dark":  {"bg": "#002b36", "fg": "#839496", "mode": "dark"},
    "solarized_light": {"bg": "#fdf6e3", "fg": "#657b83", "mode": "light"},
    "nord":            {"bg": "#2e3440", "fg": "#d8dee9", "mode": "dark"},
    "dracula":         {"bg": "#282a36", "fg": "#f8f8f2", "mode": "dark"},
    "gruvbox_dark":    {"bg": "#282828", "fg": "#ebdbb2", "mode": "dark"},
    "gruvbox_light":   {"bg": "#fbf1c7", "fg": "#3c3836", "mode": "light"},
    "one_dark":        {"bg": "#282c34", "fg": "#abb2bf", "mode": "dark"},
    "custom":          {"bg": "#1e1e1e", "fg": "#d4d4d4", "mode": "dark"},
}


def load_config():
    if os.path.exists(CONFIG_FILE):
        with open(CONFIG_FILE, "r") as f:
            saved = json.load(f)
        config = {**DEFAULTS, **saved}
        return config
    return dict(DEFAULTS)


def save_config(config):
    os.makedirs(CONFIG_DIR, exist_ok=True)
    with open(CONFIG_FILE, "w") as f:
        json.dump(config, f, indent=2)

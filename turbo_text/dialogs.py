import tkinter as tk
from tkinter import colorchooser

import customtkinter as ctk

from turbo_text.config import THEMES


FONT_FAMILIES = [
    "Consolas", "Courier New", "Cascadia Code", "Segoe UI",
    "Arial", "Lucida Console", "Verdana", "Tahoma",
    "Times New Roman", "Georgia",
]
FONT_SIZES = [10, 12, 14, 16, 18, 20, 24, 28, 32, 36]
PREVIEW_TEXT = "The quick brown fox jumps over the lazy dog.\nABCDEFGHIJKLMNOPQRSTUVWXYZ\nabcdefghijklmnopqrstuvwxyz\n0123456789 !@#$%^&*()"

THEME_DISPLAY_NAMES = {
    "dark": "Dark",
    "light": "Light",
    "monokai": "Monokai",
    "solarized_dark": "Solarized Dark",
    "solarized_light": "Solarized Light",
    "nord": "Nord",
    "dracula": "Dracula",
    "gruvbox_dark": "Gruvbox Dark",
    "gruvbox_light": "Gruvbox Light",
    "one_dark": "One Dark",
    "custom": "Custom",
}


class CustomizeDialog(ctk.CTkToplevel):
    def __init__(self, parent, config, on_apply):
        super().__init__(parent)
        self.title("Customize")
        self.geometry("520x620")
        self.resizable(False, False)
        self.transient(parent)
        self.grab_set()

        self._config = config
        self._on_apply = on_apply
        self._orig = {
            "font_family": config["font_family"],
            "font_size": config["font_size"],
            "theme": config["theme"],
            "bg_color": config.get("bg_color", ""),
            "fg_color": config.get("fg_color", ""),
        }

        # Resolve current colors
        theme_key = config["theme"]
        theme_data = THEMES.get(theme_key, THEMES["dark"])
        self._bg_color = config.get("bg_color") or theme_data["bg"]
        self._fg_color = config.get("fg_color") or theme_data["fg"]

        self._build_ui()
        self._update_preview()

        self.update_idletasks()
        px = parent.winfo_x() + (parent.winfo_width() - self.winfo_width()) // 2
        py = parent.winfo_y() + (parent.winfo_height() - self.winfo_height()) // 2
        self.geometry(f"+{px}+{py}")

    def _build_ui(self):
        scrollable = ctk.CTkScrollableFrame(self, corner_radius=0)
        scrollable.pack(fill="both", expand=True, padx=0, pady=0)

        # ── Theme ─────────────────────────────────────────────────────
        theme_frame = ctk.CTkFrame(scrollable)
        theme_frame.pack(fill="x", padx=16, pady=(16, 8))

        ctk.CTkLabel(theme_frame, text="Theme", font=("", 13, "bold")).pack(
            anchor="w", padx=12, pady=(8, 4)
        )

        # Theme preset grid
        theme_grid = ctk.CTkFrame(theme_frame, fg_color="transparent")
        theme_grid.pack(fill="x", padx=8, pady=(0, 8))

        display_names = list(THEME_DISPLAY_NAMES.values())
        theme_keys = list(THEME_DISPLAY_NAMES.keys())
        self._theme_var = ctk.StringVar(value=self._config["theme"])

        for i, (key, name) in enumerate(THEME_DISPLAY_NAMES.items()):
            if key == "custom":
                continue
            colors = THEMES[key]
            btn = ctk.CTkButton(
                theme_grid, text=name, width=100, height=32,
                fg_color=colors["bg"], text_color=colors["fg"],
                hover_color=colors["bg"],
                border_width=2, border_color="#555555",
                command=lambda k=key: self._select_theme(k),
            )
            row, col = divmod(i, 4)
            btn.grid(row=row, column=col, padx=4, pady=4)

        # ── Custom colors ─────────────────────────────────────────────
        color_frame = ctk.CTkFrame(scrollable)
        color_frame.pack(fill="x", padx=16, pady=8)

        ctk.CTkLabel(color_frame, text="Custom Colors", font=("", 13, "bold")).pack(
            anchor="w", padx=12, pady=(8, 4)
        )

        color_row = ctk.CTkFrame(color_frame, fg_color="transparent")
        color_row.pack(fill="x", padx=12, pady=(0, 8))

        # Background color
        ctk.CTkLabel(color_row, text="Background:").grid(row=0, column=0, sticky="w", pady=4)
        self._bg_swatch = tk.Canvas(color_row, width=36, height=24,
                                     highlightthickness=1, highlightbackground="#888", cursor="hand2")
        self._bg_swatch.grid(row=0, column=1, padx=(8, 4), pady=4)
        self._bg_swatch.bind("<Button-1>", lambda e: self._pick_color("bg"))
        self._bg_label = ctk.CTkLabel(color_row, text=self._bg_color, width=80)
        self._bg_label.grid(row=0, column=2, padx=4, pady=4)
        ctk.CTkButton(color_row, text="Pick...", width=60, height=24,
                       command=lambda: self._pick_color("bg")).grid(row=0, column=3, padx=4, pady=4)

        # Foreground color
        ctk.CTkLabel(color_row, text="Text:").grid(row=1, column=0, sticky="w", pady=4)
        self._fg_swatch = tk.Canvas(color_row, width=36, height=24,
                                     highlightthickness=1, highlightbackground="#888", cursor="hand2")
        self._fg_swatch.grid(row=1, column=1, padx=(8, 4), pady=4)
        self._fg_swatch.bind("<Button-1>", lambda e: self._pick_color("fg"))
        self._fg_label = ctk.CTkLabel(color_row, text=self._fg_color, width=80)
        self._fg_label.grid(row=1, column=2, padx=4, pady=4)
        ctk.CTkButton(color_row, text="Pick...", width=60, height=24,
                       command=lambda: self._pick_color("fg")).grid(row=1, column=3, padx=4, pady=4)

        self._update_swatches()

        # ── Font family ───────────────────────────────────────────────
        font_frame = ctk.CTkFrame(scrollable)
        font_frame.pack(fill="x", padx=16, pady=8)

        ctk.CTkLabel(font_frame, text="Font", font=("", 13, "bold")).pack(
            anchor="w", padx=12, pady=(8, 4)
        )
        self._font_var = ctk.StringVar(value=self._config["font_family"])
        ctk.CTkOptionMenu(
            font_frame, variable=self._font_var, values=FONT_FAMILIES,
            command=lambda _: self._on_change(), width=220,
        ).pack(anchor="w", padx=12, pady=(0, 8))

        # ── Font size ────────────────────────────────────────────────
        size_frame = ctk.CTkFrame(scrollable)
        size_frame.pack(fill="x", padx=16, pady=8)

        ctk.CTkLabel(size_frame, text="Font Size", font=("", 13, "bold")).pack(
            anchor="w", padx=12, pady=(8, 4)
        )
        self._size_var = ctk.StringVar(value=str(self._config["font_size"]))
        ctk.CTkOptionMenu(
            size_frame, variable=self._size_var,
            values=[str(s) for s in FONT_SIZES],
            command=lambda _: self._on_change(), width=120,
        ).pack(anchor="w", padx=12, pady=(0, 8))

        # ── Preview ──────────────────────────────────────────────────
        ctk.CTkLabel(scrollable, text="Preview", font=("", 13, "bold")).pack(
            anchor="w", padx=28, pady=(8, 2)
        )
        self._preview = tk.Text(
            scrollable, height=4, wrap="word", borderwidth=1, relief="solid",
            highlightthickness=0, padx=8, pady=8, state="disabled",
        )
        self._preview.pack(fill="x", padx=16, pady=(0, 12))

        # ── Buttons ──────────────────────────────────────────────────
        btn_row = ctk.CTkFrame(self, fg_color="transparent")
        btn_row.pack(fill="x", padx=16, pady=(4, 16))

        ctk.CTkButton(btn_row, text="Cancel", width=90, fg_color="gray",
                       command=self._cancel).pack(side="right", padx=(8, 0))
        ctk.CTkButton(btn_row, text="OK", width=90,
                       command=self._ok).pack(side="right")

    def _select_theme(self, key):
        self._theme_var.set(key)
        colors = THEMES[key]
        self._bg_color = colors["bg"]
        self._fg_color = colors["fg"]
        self._update_swatches()
        self._on_change()

    def _pick_color(self, which):
        initial = self._bg_color if which == "bg" else self._fg_color
        result = colorchooser.askcolor(color=initial, parent=self,
                                        title=f"Choose {'Background' if which == 'bg' else 'Text'} Color")
        if result and result[1]:
            self._theme_var.set("custom")
            if which == "bg":
                self._bg_color = result[1]
            else:
                self._fg_color = result[1]
            self._update_swatches()
            self._on_change()

    def _update_swatches(self):
        self._bg_swatch.configure(bg=self._bg_color)
        self._fg_swatch.configure(bg=self._fg_color)
        self._bg_label.configure(text=self._bg_color)
        self._fg_label.configure(text=self._fg_color)

    def _on_change(self):
        self._update_preview()

    def _update_preview(self):
        family = self._font_var.get()
        size = int(self._size_var.get())

        self._preview.configure(state="normal", bg=self._bg_color, fg=self._fg_color,
                                insertbackground=self._fg_color, font=(family, size))
        self._preview.delete("1.0", "end")
        self._preview.insert("1.0", PREVIEW_TEXT)
        self._preview.configure(state="disabled")

    def _ok(self):
        theme = self._theme_var.get()
        if theme == "custom":
            bg_color = self._bg_color
            fg_color = self._fg_color
        else:
            bg_color = ""
            fg_color = ""

        self._on_apply(
            font_family=self._font_var.get(),
            font_size=int(self._size_var.get()),
            theme=theme,
            bg_color=bg_color,
            fg_color=fg_color,
        )
        self.destroy()

    def _cancel(self):
        self._on_apply(
            font_family=self._orig["font_family"],
            font_size=self._orig["font_size"],
            theme=self._orig["theme"],
            bg_color=self._orig["bg_color"],
            fg_color=self._orig["fg_color"],
        )
        self.destroy()


class SetDefaultDialog(ctk.CTkToplevel):
    def __init__(self, parent):
        super().__init__(parent)
        self.title("Set Default Application")
        self.geometry("400x340")
        self.resizable(False, False)
        self.transient(parent)
        self.grab_set()

        self._build_ui()

        self.update_idletasks()
        px = parent.winfo_x() + (parent.winfo_width() - self.winfo_width()) // 2
        py = parent.winfo_y() + (parent.winfo_height() - self.winfo_height()) // 2
        self.geometry(f"+{px}+{py}")

    def _build_ui(self):
        # ── Common extensions ─────────────────────────────────────────
        common_frame = ctk.CTkFrame(self)
        common_frame.pack(fill="x", padx=16, pady=(16, 8))

        ctk.CTkLabel(common_frame, text="Common Extensions", font=("", 13, "bold")).pack(
            anchor="w", padx=12, pady=(8, 4)
        )

        btn_grid = ctk.CTkFrame(common_frame, fg_color="transparent")
        btn_grid.pack(fill="x", padx=8, pady=(0, 10))

        extensions = [".txt", ".md", ".log", ".cfg", ".ini", ".json", ".xml", ".csv"]
        for i, ext in enumerate(extensions):
            btn = ctk.CTkButton(
                btn_grid, text=ext, width=70, height=28,
                command=lambda e=ext: self._set_ext(e),
            )
            row, col = divmod(i, 4)
            btn.grid(row=row, column=col, padx=4, pady=4)

        # ── Custom extension ──────────────────────────────────────────
        custom_frame = ctk.CTkFrame(self)
        custom_frame.pack(fill="x", padx=16, pady=8)

        ctk.CTkLabel(custom_frame, text="Custom Extension", font=("", 13, "bold")).pack(
            anchor="w", padx=12, pady=(8, 4)
        )

        input_row = ctk.CTkFrame(custom_frame, fg_color="transparent")
        input_row.pack(fill="x", padx=12, pady=(0, 10))

        self._ext_entry = ctk.CTkEntry(input_row, width=160, placeholder_text=".myext")
        self._ext_entry.pack(side="left", padx=(0, 8))
        ctk.CTkButton(
            input_row, text="Set Default", width=100,
            command=self._set_custom,
        ).pack(side="left")

        # ── Status ────────────────────────────────────────────────────
        self._status_label = ctk.CTkLabel(self, text="", font=("", 12))
        self._status_label.pack(anchor="w", padx=28, pady=(4, 0))

        # ── Close ─────────────────────────────────────────────────────
        ctk.CTkButton(self, text="Close", width=90, fg_color="gray",
                       command=self.destroy).pack(pady=(12, 16))

    def _set_ext(self, ext):
        self._do_set_default(ext)

    def _set_custom(self):
        ext = self._ext_entry.get().strip()
        if not ext:
            return
        if not ext.startswith("."):
            ext = "." + ext
        self._do_set_default(ext)

    def _do_set_default(self, ext):
        from turbo_text.file_assoc import set_default_for

        self._status_label.configure(text=f"Setting default for {ext}...")
        self.update_idletasks()

        result = set_default_for(ext)
        if result:
            self._status_label.configure(text=f"TurboText is now the default for {ext}")
        else:
            self._status_label.configure(text=f"Requested admin for {ext} (check UAC prompt)")

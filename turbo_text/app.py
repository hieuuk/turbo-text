import os
import sys
import tkinter as tk
from tkinter import filedialog, messagebox, font as tkfont

import customtkinter as ctk
from tkinterdnd2 import DND_FILES, TkinterDnD

from turbo_text.config import load_config, save_config, THEMES
from turbo_text.markdown import apply_highlighting, configure_tags


def _icon_path():
    if getattr(sys, "frozen", False):
        base = sys._MEIPASS
    else:
        base = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    return os.path.join(base, "folder.ico")


class TurboText(ctk.CTk, TkinterDnD.DnDWrapper):
    def __init__(self, open_file=None):
        super().__init__()
        self.TkdndVersion = TkinterDnD._require(self)

        self.config = load_config()
        self.current_file = None
        self.modified = False

        theme_mode = THEMES.get(self.config["theme"], THEMES["dark"])["mode"]
        ctk.set_appearance_mode(theme_mode)
        ctk.set_default_color_theme("blue")

        self.title("Turbo Text")
        self.geometry("900x620")
        self.minsize(500, 350)

        ico = _icon_path()
        if os.path.exists(ico):
            self.iconbitmap(ico)
            self.after(200, lambda: self.iconbitmap(ico))

        self._build_menu()
        self._build_editor()
        self._apply_font()
        self._update_title()

        self.protocol("WM_DELETE_WINDOW", self._on_close)

        if open_file and os.path.isfile(open_file):
            self._open_path(open_file)

    # ── Menu ──────────────────────────────────────────────────────────

    def _build_menu(self):
        self._menu_bar = tk.Menu(self, tearoff=False)
        self.configure(menu=self._menu_bar)

        file_menu = tk.Menu(self._menu_bar, tearoff=False)
        file_menu.add_command(label="New", accelerator="Ctrl+N", command=self._new)
        file_menu.add_command(label="Open...", accelerator="Ctrl+O", command=self._open)
        file_menu.add_separator()
        file_menu.add_command(label="Save", accelerator="Ctrl+S", command=self._save)
        file_menu.add_command(label="Save As...", accelerator="Ctrl+Shift+S", command=self._save_as)
        file_menu.add_separator()
        file_menu.add_command(label="Close", accelerator="Ctrl+W", command=self._on_close)
        self._menu_bar.add_cascade(label="File", menu=file_menu)

        edit_menu = tk.Menu(self._menu_bar, tearoff=False)
        edit_menu.add_command(label="Find...", accelerator="Ctrl+F", command=self._show_find)
        self._menu_bar.add_cascade(label="Edit", menu=edit_menu)

        self._line_num_var = tk.BooleanVar(value=self.config.get("show_line_numbers", True))
        self._word_wrap_var = tk.BooleanVar(value=self.config.get("word_wrap", True))

        settings_menu = tk.Menu(self._menu_bar, tearoff=False)
        settings_menu.add_checkbutton(
            label="Show Line Numbers", variable=self._line_num_var,
            command=self._toggle_line_numbers,
        )
        settings_menu.add_checkbutton(
            label="Word Wrap", variable=self._word_wrap_var,
            command=self._toggle_word_wrap,
        )
        settings_menu.add_separator()
        settings_menu.add_command(label="Set Default App...", command=self._show_set_default)
        settings_menu.add_command(label="Customize...", command=self._show_customize)
        self._menu_bar.add_cascade(label="Settings", menu=settings_menu)

        # Keyboard shortcuts
        self.bind_all("<Control-n>", lambda e: self._new())
        self.bind_all("<Control-o>", lambda e: self._open())
        self.bind_all("<Control-s>", lambda e: self._save())
        self.bind_all("<Control-Shift-S>", lambda e: self._save_as())
        self.bind_all("<Control-w>", lambda e: self._on_close())
        self.bind_all("<Control-f>", lambda e: self._show_find())

    # ── Editor ────────────────────────────────────────────────────────

    def _build_editor(self):
        self._editor_frame = ctk.CTkFrame(self, corner_radius=0)
        self._editor_frame.pack(fill="both", expand=True, padx=0, pady=0)

        wrap_mode = "word" if self.config.get("word_wrap", True) else "none"

        # Line number gutter
        self._line_numbers = tk.Text(
            self._editor_frame,
            width=4,
            padx=4,
            pady=10,
            borderwidth=0,
            highlightthickness=0,
            state="disabled",
            wrap="none",
            takefocus=0,
            cursor="arrow",
        )

        self._text = tk.Text(
            self._editor_frame,
            wrap=wrap_mode,
            undo=True,
            maxundo=-1,
            borderwidth=0,
            highlightthickness=0,
            padx=10,
            pady=10,
        )

        self._scrollbar = ctk.CTkScrollbar(self._editor_frame, command=self._on_scrollbar)
        self._text.configure(yscrollcommand=self._on_text_scroll)

        self._scrollbar.pack(side="right", fill="y")
        if self.config.get("show_line_numbers", True):
            self._line_numbers.pack(side="left", fill="y")
        self._text.pack(side="left", fill="both", expand=True)

        self._text.bind("<<Modified>>", self._on_modified)
        self._text.bind("<KeyRelease>", self._schedule_highlight)
        self._text.bind("<KeyRelease>", self._schedule_line_update, add=True)
        self._text.bind("<ButtonRelease-1>", self._schedule_line_update)
        self._text.bind("<Configure>", self._schedule_line_update)
        self._text.bind("<MouseWheel>", self._schedule_line_update)
        self._highlight_job = None
        self._line_update_job = None

        # Find bar (hidden initially)
        self._find_frame = ctk.CTkFrame(self, corner_radius=0, height=36)
        self._find_entry = ctk.CTkEntry(self._find_frame, placeholder_text="Find...", width=260)
        self._find_entry.pack(side="left", padx=(8, 4), pady=4)
        self._find_entry.bind("<Return>", lambda e: self._find_next())
        self._find_entry.bind("<Escape>", lambda e: self._close_find())

        ctk.CTkButton(self._find_frame, text="Next", width=60, command=self._find_next).pack(side="left", padx=2, pady=4)
        ctk.CTkButton(self._find_frame, text="Prev", width=60, command=self._find_prev).pack(side="left", padx=2, pady=4)
        self._find_case_var = tk.BooleanVar(value=False)
        ctk.CTkCheckBox(self._find_frame, text="Match Case", variable=self._find_case_var, width=100).pack(side="left", padx=(8, 4), pady=4)
        ctk.CTkButton(self._find_frame, text="✕", width=30, command=self._close_find).pack(side="right", padx=(2, 8), pady=4)
        self._find_visible = False

        self.drop_target_register(DND_FILES)
        self.dnd_bind("<<Drop>>", self._on_drop)

        self._apply_editor_colors()
        self.after(50, self._update_line_numbers)

    def _resolve_colors(self):
        theme_key = self.config["theme"]
        theme_data = THEMES.get(theme_key, THEMES["dark"])
        bg = self.config.get("bg_color") or theme_data["bg"]
        fg = self.config.get("fg_color") or theme_data["fg"]
        return bg, fg, theme_data["mode"]

    def _apply_editor_colors(self):
        bg, fg, mode = self._resolve_colors()
        if mode == "dark":
            select_bg = "#264f78"
            gutter_bg = "#252526"
            gutter_fg = "#858585"
        else:
            select_bg = "#add6ff"
            gutter_bg = "#f3f3f3"
            gutter_fg = "#237893"

        self._text.configure(
            bg=bg, fg=fg, insertbackground=fg, selectbackground=select_bg
        )
        self._line_numbers.configure(bg=gutter_bg, fg=gutter_fg, selectbackground=gutter_bg, selectforeground=gutter_fg)

    def _apply_font(self):
        fnt = (self.config["font_family"], int(self.config["font_size"]))
        self._text.configure(font=fnt)
        self._line_numbers.configure(font=fnt)
        configure_tags(self._text, self.config["font_family"], self.config["font_size"])
        self._run_highlight()
        self._update_line_numbers()

    # ── Line numbers ─────────────────────────────────────────────────

    def _on_scrollbar(self, *args):
        self._text.yview(*args)
        self._line_numbers.yview(*args)

    def _on_text_scroll(self, first, last):
        self._scrollbar.set(first, last)
        self._line_numbers.yview_moveto(first)

    def _schedule_line_update(self, event=None):
        if self._line_update_job:
            self.after_cancel(self._line_update_job)
        self._line_update_job = self.after(30, self._update_line_numbers)

    def _update_line_numbers(self):
        self._line_update_job = None
        if not self.config.get("show_line_numbers", True):
            return

        self._line_numbers.configure(state="normal")
        self._line_numbers.delete("1.0", "end")

        line_count = int(self._text.index("end-1c").split(".")[0])
        width = max(4, len(str(line_count)) + 1)
        self._line_numbers.configure(width=width)

        lines = "\n".join(str(i) for i in range(1, line_count + 1))
        self._line_numbers.insert("1.0", lines)
        self._line_numbers.configure(state="disabled")

        # Sync scroll position
        first = self._text.yview()[0]
        self._line_numbers.yview_moveto(first)

    def _toggle_line_numbers(self):
        show = self._line_num_var.get()
        self.config["show_line_numbers"] = show
        save_config(self.config)
        if show:
            self._line_numbers.pack(side="left", fill="y", before=self._text)
            self._update_line_numbers()
        else:
            self._line_numbers.pack_forget()

    # ── Word wrap ────────────────────────────────────────────────────

    def _toggle_word_wrap(self):
        wrap = self._word_wrap_var.get()
        self.config["word_wrap"] = wrap
        save_config(self.config)
        self._text.configure(wrap="word" if wrap else "none")
        self._update_line_numbers()

    # ── Find ─────────────────────────────────────────────────────────

    def _show_find(self):
        if not self._find_visible:
            self._find_frame.pack(side="bottom", fill="x", before=self._editor_frame)
            self._find_visible = True
        self._find_entry.focus_set()
        sel = self._text.tag_ranges("sel")
        if sel:
            self._find_entry.delete(0, "end")
            self._find_entry.insert(0, self._text.get(*sel))

    def _close_find(self):
        self._find_frame.pack_forget()
        self._find_visible = False
        self._text.tag_remove("find_highlight", "1.0", "end")
        self._text.focus_set()

    def _find_next(self):
        self._find_direction(forwards=True)

    def _find_prev(self):
        self._find_direction(forwards=False)

    def _find_direction(self, forwards=True):
        query = self._find_entry.get()
        if not query:
            return

        self._text.tag_remove("find_highlight", "1.0", "end")
        self._text.tag_configure("find_highlight", background="#515c6a", foreground="#ffffff")

        nocase = not self._find_case_var.get()

        # Highlight all matches
        start_pos = "1.0"
        while True:
            pos = self._text.search(query, start_pos, stopindex="end", nocase=nocase)
            if not pos:
                break
            end_pos = f"{pos}+{len(query)}c"
            self._text.tag_add("find_highlight", pos, end_pos)
            start_pos = end_pos

        # Navigate to next/prev match from cursor
        cursor = self._text.index("insert")
        if forwards:
            pos = self._text.search(query, f"{cursor}+1c", stopindex="end", nocase=nocase)
            if not pos:
                pos = self._text.search(query, "1.0", stopindex="end", nocase=nocase)
        else:
            pos = self._text.search(query, cursor, stopindex="1.0", backwards=True, nocase=nocase)
            if not pos:
                pos = self._text.search(query, "end", stopindex="1.0", backwards=True, nocase=nocase)

        if pos:
            end_pos = f"{pos}+{len(query)}c"
            self._text.tag_remove("sel", "1.0", "end")
            self._text.tag_add("sel", pos, end_pos)
            self._text.mark_set("insert", pos)
            self._text.see(pos)

    # ── File operations ───────────────────────────────────────────────

    def _new(self):
        if not self._check_save():
            return
        self._text.delete("1.0", "end")
        self.current_file = None
        self.modified = False
        self._text.edit_modified(False)
        self._update_title()

    def _open(self):
        if not self._check_save():
            return
        path = filedialog.askopenfilename(
            initialdir=self.config.get("last_directory") or None,
            filetypes=[
                ("Text & Markdown", "*.txt *.md"),
                ("Text files", "*.txt"),
                ("Markdown files", "*.md"),
                ("All files", "*.*"),
            ],
        )
        if path:
            self._open_path(path)

    def _open_path(self, path):
        try:
            with open(path, "r", encoding="utf-8", errors="replace") as f:
                content = f.read()
        except Exception as e:
            messagebox.showerror("Error", f"Could not open file:\n{e}")
            return

        self._text.delete("1.0", "end")
        self._text.insert("1.0", content)
        self._text.edit_modified(False)
        self.current_file = path
        self.modified = False
        self.config["last_directory"] = os.path.dirname(path)
        save_config(self.config)
        self._update_title()
        self._apply_font()

    def _save(self):
        if self.current_file:
            self._write_file(self.current_file)
        else:
            self._save_as()

    def _save_as(self):
        path = filedialog.asksaveasfilename(
            initialdir=self.config.get("last_directory") or None,
            defaultextension=".txt",
            filetypes=[
                ("Text files", "*.txt"),
                ("Markdown files", "*.md"),
                ("All files", "*.*"),
            ],
        )
        if path:
            self._write_file(path)

    def _write_file(self, path):
        try:
            content = self._text.get("1.0", "end-1c")
            with open(path, "w", encoding="utf-8") as f:
                f.write(content)
        except Exception as e:
            messagebox.showerror("Error", f"Could not save file:\n{e}")
            return

        self.current_file = path
        self.modified = False
        self._text.edit_modified(False)
        self.config["last_directory"] = os.path.dirname(path)
        save_config(self.config)
        self._update_title()

    # ── Settings / Customization ──────────────────────────────────────

    def _show_customize(self):
        from turbo_text.dialogs import CustomizeDialog
        CustomizeDialog(self, self.config, self._apply_customization)

    def _apply_customization(self, font_family, font_size, theme, bg_color="", fg_color=""):
        self.config["font_family"] = font_family
        self.config["font_size"] = font_size
        self.config["theme"] = theme
        self.config["bg_color"] = bg_color
        self.config["fg_color"] = fg_color
        save_config(self.config)
        theme_mode = THEMES.get(theme, THEMES["dark"])["mode"]
        ctk.set_appearance_mode(theme_mode)
        self._apply_editor_colors()
        self._apply_font()

    def _show_set_default(self):
        from turbo_text.dialogs import SetDefaultDialog
        SetDefaultDialog(self)

    # ── Helpers ───────────────────────────────────────────────────────

    def _schedule_highlight(self, event=None):
        if self._highlight_job:
            self.after_cancel(self._highlight_job)
        self._highlight_job = self.after(150, self._run_highlight)

    def _is_markdown(self):
        return self.current_file and self.current_file.lower().endswith(".md")

    def _run_highlight(self):
        self._highlight_job = None
        if self._is_markdown():
            _, _, mode = self._resolve_colors()
            apply_highlighting(self._text, mode)
        else:
            from turbo_text.markdown import ALL_TAGS
            for tag in ALL_TAGS + ["inline_code_light"]:
                self._text.tag_remove(tag, "1.0", "end")

    def _on_modified(self, event=None):
        if self._text.edit_modified():
            self.modified = True
            self._update_title()

    def _update_title(self):
        name = os.path.basename(self.current_file) if self.current_file else "Untitled"
        marker = " *" if self.modified else ""
        self.title(f"{name}{marker} - Turbo Text")

    def _check_save(self):
        """Returns True if safe to proceed, False to cancel."""
        if not self.modified:
            return True
        result = messagebox.askyesnocancel(
            "Unsaved Changes", "You have unsaved changes. Save before continuing?"
        )
        if result is None:
            return False
        if result:
            self._save()
        return True

    def _on_drop(self, event):
        path = event.data.strip()
        if path.startswith("{") and path.endswith("}"):
            path = path[1:-1]
        if os.path.isfile(path):
            if not self._check_save():
                return
            self._open_path(path)

    def _on_close(self):
        if self._check_save():
            self.destroy()

import sys

from turbo_text.app import TurboText


def main():
    # Handle elevated admin operations
    if len(sys.argv) >= 3 and sys.argv[1] == "--set-default":
        from turbo_text.file_assoc import set_default_for

        set_default_for(sys.argv[2])
        return

    if len(sys.argv) >= 2 and sys.argv[1] == "--add-context-menu":
        from turbo_text.file_assoc import add_context_menu

        add_context_menu()
        return

    if len(sys.argv) >= 2 and sys.argv[1] == "--remove-context-menu":
        from turbo_text.file_assoc import remove_context_menu

        remove_context_menu()
        return

    # Normal launch — optionally open a file passed as argument
    open_file = sys.argv[1] if len(sys.argv) >= 2 else None
    app = TurboText(open_file=open_file)
    app.mainloop()


if __name__ == "__main__":
    main()

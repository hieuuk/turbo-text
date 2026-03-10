import sys

from turbo_text.app import TurboText


def main():
    # Handle --set-default flag (runs elevated for file association)
    if len(sys.argv) >= 3 and sys.argv[1] == "--set-default":
        from turbo_text.file_assoc import set_default_for

        set_default_for(sys.argv[2])
        return

    # Normal launch — optionally open a file passed as argument
    open_file = sys.argv[1] if len(sys.argv) >= 2 else None
    app = TurboText(open_file=open_file)
    app.mainloop()


if __name__ == "__main__":
    main()

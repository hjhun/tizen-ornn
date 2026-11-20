case ":$PATH:" in
    *:/usr/local/bin:*) ;;
    *) PATH="/usr/local/bin:$PATH" ;;
esac
export PATH

# Enter the administrative shell for scenarios that exercise system services.
proc root_login {} {
    global spawn_id timeout
    expect {
        -re {tizen(?:@[^\r\n]*\$|:[^\r\n]*>) $} {}
        timeout {error "Guest developer login timed out"}
        eof {error "Guest exited before developer login"}
    }
    send -- "su -\r"
    expect {
        -re {root:[^\r\n]*> $} {}
        timeout {error "Guest root login timed out"}
        eof {error "Guest exited before root login"}
    }
}

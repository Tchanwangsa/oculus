"""Embedded WSL2 Claude supervisor. All caller data arrives as JSON/argv.

The outer bwrap protects Windows files even from Claude's built-in tools. Its
fresh PID namespace, hidden WSL interop endpoints and seccomp filter also apply
to Claude itself. The inner Claude sandbox still scopes individual Bash tools.
Windows owns stdin and sends heartbeats; EOF or a missed heartbeat tears down
the whole Linux PID namespace, including detached grandchildren.
"""
import ctypes
import errno
import json
import os
import pathlib
import platform
import queue
import selectors
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time
import uuid

MAX_LINE = 8 * 1024 * 1024
MAX_REQUESTS = 4
HEARTBEAT_TIMEOUT = 15


def linux_environment(home):
    # Never import Windows PATH, WSL interop handles, shell startup hooks, API
    # credentials, or nested-Claude flags. Claude uses its Linux subscription.
    return {
        "HOME": home,
        "USER": pathlib.Path(home).name,
        "LOGNAME": pathlib.Path(home).name,
        "PATH": f"{home}/.local/bin:/usr/local/bin:/usr/bin:/bin",
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
        "TERM": "dumb",
        "CLAUDE_CODE_ENTRYPOINT": "cli",
        "DISABLE_AUTOUPDATER": "1",
    }


def runtime():
    release = platform.release().lower()
    if "microsoft" not in release or "wsl2" not in release:
        raise RuntimeError("Claude requires a WSL2 distribution, not WSL1.")
    if os.getuid() == 0:
        raise RuntimeError("Set a non-root default user in this WSL2 distribution before using Claude.")
    home = str(pathlib.Path.home().resolve())
    env = linux_environment(home)
    claude = shutil.which("claude", path=env["PATH"])
    if not claude:
        raise RuntimeError("Install Claude Code inside this WSL2 distribution, then sign in with `claude auth login`.")
    claude = os.path.realpath(claude)
    with open(claude, "rb") as executable:
        if executable.read(4) != b"\x7fELF":
            raise RuntimeError("Install the native Linux Claude Code executable; Windows executables and shell wrappers are not supported.")
    for dependency in ("bwrap", "socat", "python3"):
        if not shutil.which(dependency, path=env["PATH"]):
            raise RuntimeError(f"Install {dependency} in this WSL2 distribution to enable Claude's sandbox.")
    ctypes.CDLL("libseccomp.so.2")
    version = subprocess.run([claude, "--version"], env=env, capture_output=True, text=True, timeout=20)
    if version.returncode:
        raise RuntimeError("The Linux Claude executable could not start.")
    return {"home": home, "config": home + "/.claude", "claude": claude,
            "version": version.stdout.strip(), "env": env}


def probe():
    info = runtime()
    # A dependency existing is insufficient: WSL/AppArmor can prohibit user
    # namespaces. Fail closed before offering this bridge in the UI.
    test = subprocess.run(["bwrap", "--die-with-parent", "--unshare-all", "--share-net",
                           "--ro-bind", "/", "/", "--proc", "/proc", "--dev", "/dev",
                           "--", "/usr/bin/true"], env=info["env"], capture_output=True, timeout=15)
    if test.returncode:
        raise RuntimeError("Bubblewrap cannot create the required sandbox in this WSL2 distribution. Check its user-namespace/AppArmor configuration.")
    auth = subprocess.run([info["claude"], "auth", "status", "--json"], env=info["env"],
                          capture_output=True, text=True, timeout=20)
    try:
        logged_in = json.loads(auth.stdout).get("loggedIn") is True
    except (ValueError, AttributeError):
        logged_in = False
    return {key: info[key] for key in ("home", "config", "claude", "version")} | {"logged_in": logged_in}


def auth_login(info=None):
    """Only the vendor login, without an agent session or any library access.

    Windows owns stdin. EOF cancels even if the CLI has already stopped
    reading input; the child cannot outlive an abandoned sign-in dialog.
    """
    info = runtime() if info is None else info
    child = subprocess.Popen([info["claude"], "auth", "login"],
                             env=info["env"], cwd=info["home"],
                             stdin=subprocess.PIPE, start_new_session=True)
    stopped = threading.Event()

    def input_loop():
        try:
            # A daemon blocked in BufferedReader.readline holds Python's
            # stdin lock during interpreter shutdown. Read the descriptor
            # directly so a successful login can exit while Windows still
            # owns the pipe and has not closed the dialog yet.
            while True:
                chunk = os.read(sys.stdin.fileno(), 4096)
                if not chunk:
                    break
                child.stdin.write(chunk)
                child.stdin.flush()
        except (BrokenPipeError, OSError):
            pass
        finally:
            stopped.set()

    threading.Thread(target=input_loop, daemon=True).start()
    try:
        while child.poll() is None and not stopped.wait(0.1):
            pass
    finally:
        if child.poll() is None:
            try:
                os.killpg(child.pid, signal.SIGTERM)
                child.wait(timeout=2)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=5)
            except ProcessLookupError:
                pass
    return child.returncode if child.returncode is not None and child.returncode >= 0 else 1


def models(cwd, info=None, timeout=18):
    """Free catalogue handshake, never a user turn or a persisted session.

    Windows owns stdin, but only initialize is accepted. Its EOF cancels the
    Linux process group; a deadline also bounds a wedged CLI even if wsl.exe
    is killed before Windows closes the duplicated input pipe.
    """
    request = json.loads(sys.stdin.buffer.readline(MAX_LINE))
    if (not isinstance(request, dict) or request.get("type") != "control_request"
            or request.get("request_id") != "oculus-models"
            or request.get("request") != {"subtype": "initialize"}):
        raise RuntimeError("The catalogue accepts only an initialize request.")
    info = runtime() if info is None else info
    cwd = str(pathlib.Path(cwd).resolve(strict=True))
    child = subprocess.Popen([
        info["claude"], "-p", "--input-format", "stream-json",
        "--output-format", "stream-json", "--verbose", "--no-session-persistence",
        "--strict-mcp-config", "--settings", '{"disableAllHooks":true}',
    ], cwd=cwd, env=info["env"], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, start_new_session=True)
    stopped = threading.Event()
    answered = threading.Event()

    def watch_owner():
        # No other input is permitted. os.read avoids retaining a buffered
        # stdin lock when this daemon is abandoned on interpreter shutdown.
        try:
            os.read(sys.stdin.fileno(), 1)
        except OSError:
            pass
        stopped.set()

    def read_answer():
        try:
            for line in iter(child.stdout.readline, b""):
                try:
                    value = json.loads(line)
                except ValueError:
                    continue
                if (isinstance(value, dict) and value.get("type") == "control_response"
                        and isinstance(value.get("response"), dict)
                        and value["response"].get("request_id") == "oculus-models"):
                    sys.stdout.buffer.write(line)
                    sys.stdout.buffer.flush()
                    answered.set()
                    break
        finally:
            stopped.set()

    try:
        child.stdin.write(json.dumps(request).encode("utf-8") + b"\n")
        child.stdin.flush()
        threading.Thread(target=watch_owner, daemon=True).start()
        threading.Thread(target=read_answer, daemon=True).start()
        if not stopped.wait(timeout):
            raise RuntimeError("Claude did not list its models before the Linux deadline.")
        return 0 if answered.is_set() else 1
    finally:
        # Kill the group even if its leader just exited: an inherited stdout
        # pipe must not keep the Windows reader alive after this operation.
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        child.wait(timeout=5)


def seccomp_file(directory):
    """libseccomp includes an architecture guard, including the x32 ABI.

    Do not block mount/unshare: Claude's nested bwrap needs them inside its
    unprivileged user namespace. Namespace capabilities never grant access
    to the host mount namespace. No-new-privileges and dropped capabilities
    remain in force. Block inspection of the supervisor and WSL host vsock.
    """
    library = ctypes.CDLL("libseccomp.so.2", use_errno=True)
    library.seccomp_init.argtypes = [ctypes.c_uint32]
    library.seccomp_init.restype = ctypes.c_void_p
    library.seccomp_release.argtypes = [ctypes.c_void_p]
    library.seccomp_syscall_resolve_name.argtypes = [ctypes.c_char_p]
    library.seccomp_syscall_resolve_name.restype = ctypes.c_int
    library.seccomp_rule_add.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.c_int, ctypes.c_uint]
    library.seccomp_export_bpf.argtypes = [ctypes.c_void_p, ctypes.c_int]
    context = library.seccomp_init(0x7FFF0000)  # SCMP_ACT_ALLOW
    if not context:
        raise RuntimeError("Cannot initialize the required seccomp filter.")
    deny = 0x00050000 | errno.EPERM
    try:
        for name in ("ptrace", "process_vm_writev", "process_vm_readv", "kcmp", "bpf", "perf_event_open", "keyctl", "add_key", "request_key"):
            syscall = library.seccomp_syscall_resolve_name(name.encode("ascii"))
            if syscall >= 0 and library.seccomp_rule_add(context, deny, syscall, 0) != 0:
                raise RuntimeError("Cannot install the required seccomp rule.")
        class Comparison(ctypes.Structure):
            _fields_ = [("arg", ctypes.c_uint), ("op", ctypes.c_int),
                        ("datum_a", ctypes.c_uint64), ("datum_b", ctypes.c_uint64)]
        library.seccomp_rule_add_array.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.c_int,
                                                  ctypes.c_uint, ctypes.POINTER(Comparison)]
        comparison = Comparison(0, 4, getattr(socket, "AF_VSOCK", 40), 0)  # SCMP_CMP_EQ
        syscall = library.seccomp_syscall_resolve_name(b"socket")
        if library.seccomp_rule_add_array(context, deny, syscall, 1, ctypes.byref(comparison)) != 0:
            raise RuntimeError("Cannot disable WSL host sockets.")
        path = os.path.join(directory, "seccomp.bpf")
        with open(path, "wb") as output:
            if library.seccomp_export_bpf(context, output.fileno()) != 0:
                raise RuntimeError("Cannot export the required seccomp filter.")
        return open(path, "rb")
    finally:
        library.seccomp_release(context)


CLI_SHIM = '''#!/usr/bin/python3
import fcntl, json, os, selectors, sys, time, uuid
try:
    argv = sys.argv[1:]
    # Capture before waiting for a slot: an interrupted/queued command may
    # never inherit permission from a later turn that happens to be running.
    with open("/run/oculus/generation", encoding="ascii") as source:
        generation = int(source.read())
    payload = {"argv": argv, "generation": generation}
    words = [word for word in argv if word != "--json"]
    if len(words) >= 2 and words[:2] == ["task", "add"]:
        for index, argument in enumerate(argv):
            source = None
            if argument == "--batch" and index + 1 < len(argv):
                source = argv[index + 1]
                argv[index + 1] = "-"
            elif argument.startswith("--batch="):
                source = argument.split("=", 1)[1]
                argv[index] = "--batch=-"
            if source is not None:
                if source == "-":
                    content = sys.stdin.buffer.read(1024 * 1024 + 1)
                else:
                    with open(source, "rb") as batch:
                        content = batch.read(1024 * 1024 + 1)
                if len(content) > 1024 * 1024: raise RuntimeError("Oculus batch input is too large")
                payload["stdin"] = content.decode("utf-8")
                break
    identifier = uuid.uuid4().hex
    payload["client_id"] = identifier
    request = b"\\x1e" + json.dumps(payload, ensure_ascii=False).encode("utf-8") + b"\\n"
    if len(request) > 2 * 1024 * 1024: raise RuntimeError("Oculus arguments are too large")
    deadline = time.monotonic() + 100
    lock = None
    while lock is None and time.monotonic() < deadline:
        for slot in range(4):
            candidate = open(f"/run/oculus/slot-{slot}.lock", "rb")
            try:
                fcntl.flock(candidate, fcntl.LOCK_EX | fcntl.LOCK_NB)
                lock = candidate
                break
            except BlockingIOError:
                candidate.close()
        if lock is None: time.sleep(0.02)
    if lock is None: raise RuntimeError("Oculus CLI is busy")
    response_fd = os.open(f"/run/oculus/response-{slot}.fifo", os.O_RDWR | os.O_NONBLOCK)
    # A prior cancelled client may leave bytes behind. The lock and per-call
    # id isolate this request even if its predecessor finishes concurrently.
    while True:
        try:
            if not os.read(response_fd, 65536): break
        except BlockingIOError:
            break
    request_fd = os.open(f"/run/oculus/request-{slot}.fifo", os.O_WRONLY | os.O_NONBLOCK)
    with selectors.DefaultSelector() as selector:
        selector.register(request_fd, selectors.EVENT_WRITE)
        position = 0
        while position < len(request):
            if time.monotonic() >= deadline: raise RuntimeError("Oculus CLI request timed out")
            if not selector.select(0.2): continue
            try: position += os.write(request_fd, request[position:position + 4096])
            except BlockingIOError: pass
        selector.unregister(request_fd)
        os.close(request_fd)
        selector.register(response_fd, selectors.EVENT_READ)
        buffer = b""
        result = None
        while result is None:
            if time.monotonic() >= deadline: raise RuntimeError("Oculus CLI response timed out")
            if not selector.select(0.2): continue
            buffer += os.read(response_fd, 65536)
            if len(buffer) > 9 * 1024 * 1024: raise RuntimeError("Oculus response is too large")
            while b"\\n" in buffer:
                line, buffer = buffer.split(b"\\n", 1)
                line = line.rsplit(b"\\x1e", 1)[-1]
                try: answer = json.loads(line)
                except ValueError: continue
                if not isinstance(answer, dict): continue
                if answer.get("client_id") == identifier:
                    result = answer
                    break
    os.close(response_fd)
    lock.close()
    sys.stdout.write(result.get("stdout", ""))
    sys.stderr.write(result.get("stderr", ""))
    sys.exit(result.get("code", 1))
except Exception as error:
    print("Oculus CLI bridge: " + str(error), file=sys.stderr)
    sys.exit(1)
'''


def sandbox_command(info, config, directory, seccomp):
    library = os.path.realpath(config["library"])
    cwd = os.path.realpath(config["cwd"])
    if cwd != os.path.join(library, "agents") or not os.path.isdir(cwd):
        raise RuntimeError("Claude's working directory must be the library's own agents folder.")
    if library == "/" or not os.path.isdir(library):
        raise RuntimeError("Invalid Oculus library path.")
    home = info["home"]
    config_dir = info["config"]
    os.makedirs(config_dir, mode=0o700, exist_ok=True)
    global_config = home + "/.claude.json"
    if not os.path.exists(global_config):
        descriptor = os.open(global_config, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
        os.write(descriptor, b"{}")
        os.close(descriptor)
    args = ["bwrap", "--die-with-parent", "--unshare-all", "--share-net",
            "--cap-drop", "ALL", "--ro-bind", "/", "/", "--proc", "/proc", "--dev", "/dev",
            "--tmpfs", "/tmp", "--tmpfs", "/run", "--tmpfs", "/mnt"]
    # The PE binfmt interpreter must not be reachable even if a Bash tool
    # copies powershell.exe under a different name into agents/.
    if os.path.exists("/init"):
        args += ["--ro-bind", "/dev/null", "/init"]
    # /mnt hides every standard Windows drive and WSL shared service socket.
    # Also hide custom drive automount locations. Expose only this library,
    # so the model cannot read unrelated Windows files via an absolute path.
    def mount_unescape(value):
        for escaped, plain in (("\\040", " "), ("\\011", "\t"), ("\\012", "\n"), ("\\134", "\\")):
            value = value.replace(escaped, plain)
        return value
    with open("/proc/self/mountinfo", encoding="utf-8") as mountinfo:
        for line in mountinfo:
            before, after = line.rstrip().split(" - ", 1)
            target = mount_unescape(before.split()[4])
            kind = after.split()[0]
            if kind in ("9p", "drvfs") and not (target == "/mnt" or target.startswith("/mnt/")):
                if target == "/":
                    raise RuntimeError("The WSL2 Linux root cannot be a Windows filesystem.")
                args += ["--tmpfs", target]
    # WSL's /etc/resolv.conf normally points into /mnt/wsl, and systemd
    # distributions may point into /run. Both parents are deliberately hidden.
    # Recreate only the resolver file from a private immutable snapshot; do not
    # expose the shared directory or its host-service sockets to the sandbox.
    resolver = os.path.realpath("/etc/resolv.conf")
    resolver_copy = os.path.join(directory, "resolv.conf")
    shutil.copyfile("/etc/resolv.conf", resolver_copy)
    args += ["--ro-bind", resolver_copy, resolver]
    args += ["--ro-bind", library, library,
             "--ro-bind", directory, "/run/oculus", "--bind", cwd, cwd,
             "--bind", config_dir, config_dir, "--bind", global_config, global_config]
    # Shipped procedures and their provider routes are generated by Oculus,
    # not model-authored memory. Bind existing folders only: a fresh library
    # need not have any of them and readonly siblings must not be created.
    for relative in ("skills", ".claude/skills", ".agents/skills"):
        protected = os.path.join(cwd, relative)
        if os.path.exists(protected):
            if os.path.commonpath((cwd, os.path.realpath(protected))) != cwd:
                raise RuntimeError("Agent skills must resolve inside the agents folder.")
            args += ["--ro-bind", protected, protected]
    args += ["--chdir", cwd, "--seccomp", str(seccomp.fileno()), "--", info["claude"]]
    args += config["args"]
    env = info["env"] | {"PATH": "/run/oculus:" + info["env"]["PATH"], "CLAUDE_CONFIG_DIR": config_dir}
    return args, env


def session():
    first = sys.stdin.buffer.readline(MAX_LINE + 1)
    if len(first) > MAX_LINE or not first.endswith(b"\n"):
        raise RuntimeError("Missing or oversized Oculus bridge configuration.")
    config = json.loads(first)
    info = runtime()
    output_lock = threading.Lock()
    pending_lock = threading.Lock()
    pending = {}
    stopped = threading.Event()
    heartbeat = [time.monotonic()]
    generation = [0]

    def emit(value):
        data = json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8") + b"\n"
        with output_lock:
            sys.stdout.buffer.write(data)
            sys.stdout.buffer.flush()

    # A Python supervisor remains outside bwrap; the bridge pipes and filter
    # are immutable inside it. Only the controlled shim crosses this boundary.
    with tempfile.TemporaryDirectory(prefix="oculus-claude-") as directory:
        os.chmod(directory, 0o700)
        shim = os.path.join(directory, "oculus")
        pathlib.Path(shim).write_text(CLI_SHIM, encoding="utf-8")
        os.chmod(shim, 0o500)
        generation_file = pathlib.Path(directory, "generation")
        generation_file.write_text("0", encoding="ascii")
        request_fds = []
        for slot in range(MAX_REQUESTS):
            pathlib.Path(directory, f"slot-{slot}.lock").touch(mode=0o400)
            request_path = os.path.join(directory, f"request-{slot}.fifo")
            os.mkfifo(request_path, 0o600)
            os.mkfifo(os.path.join(directory, f"response-{slot}.fifo"), 0o600)
            # Holding both ends avoids spurious EOF between requests. These
            # descriptors are never inherited by the sandboxed process.
            request_fds.append(os.open(request_path, os.O_RDWR | os.O_NONBLOCK))
        with seccomp_file(directory) as seccomp:
            args, env = sandbox_command(info, config, directory, seccomp)
            child = subprocess.Popen(args, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=None, start_new_session=True, pass_fds=(seccomp.fileno(),))
        child_input_lock = threading.Lock()

        def stop(*_):
            stopped.set()

        for signum in (signal.SIGTERM, signal.SIGHUP, signal.SIGINT):
            signal.signal(signum, stop)

        def input_loop():
            try:
                while not stopped.is_set():
                    line = sys.stdin.buffer.readline(MAX_LINE + 1)
                    if not line or len(line) > MAX_LINE or not line.endswith(b"\n"):
                        break
                    message = json.loads(line)
                    kind = message.get("type")
                    if kind == "oculus_ping":
                        heartbeat[0] = time.monotonic()
                    elif kind == "oculus_cli_response":
                        with pending_lock:
                            answer = pending.get(message.get("id"))
                        if answer is not None:
                            answer.put_nowait(message)
                    elif kind in ("user", "control_request"):
                        if "oculus_generation" in message:
                            current = message.pop("oculus_generation")
                            if not isinstance(current, int) or isinstance(current, bool) or current < generation[0]:
                                raise ValueError("Invalid Oculus turn generation.")
                            generation[0] = current
                            temporary = pathlib.Path(directory, "generation.next")
                            temporary.write_text(str(current), encoding="ascii")
                            os.replace(temporary, generation_file)
                            line = json.dumps(message, ensure_ascii=False).encode("utf-8") + b"\n"
                        with child_input_lock:
                            child.stdin.write(line)
                            child.stdin.flush()
                    else:
                        raise RuntimeError("Unexpected Oculus bridge input.")
            except (OSError, ValueError, queue.Full):
                pass
            finally:
                stopped.set()

        def output_loop():
            try:
                for line in iter(child.stdout.readline, b""):
                    with output_lock:
                        sys.stdout.buffer.write(line)
                        sys.stdout.buffer.flush()
            except OSError:
                pass
            finally:
                stopped.set()

        def write_response(slot, result):
            data = b"\x1e" + json.dumps(result, ensure_ascii=False).encode("utf-8") + b"\n"
            try:
                descriptor = os.open(os.path.join(directory, f"response-{slot}.fifo"), os.O_WRONLY | os.O_NONBLOCK)
            except OSError:
                return  # The client was cancelled before its response arrived.
            try:
                with selectors.DefaultSelector() as selector:
                    selector.register(descriptor, selectors.EVENT_WRITE)
                    deadline = time.monotonic() + 5
                    position = 0
                    while position < len(data) and not stopped.is_set() and time.monotonic() < deadline:
                        if not selector.select(0.2):
                            continue
                        try:
                            position += os.write(descriptor, data[position:position + 4096])
                        except BlockingIOError:
                            pass
                        except BrokenPipeError:
                            return
            finally:
                os.close(descriptor)

        def broker_request(slot, raw):
            identifier = uuid.uuid4().hex
            client_id = None
            try:
                if len(raw) > 2 * 1024 * 1024 or not raw.endswith(b"\n"):
                    raise RuntimeError("Invalid Oculus CLI request.")
                message = json.loads(raw)
                if not isinstance(message, dict):
                    raise RuntimeError("Invalid Oculus CLI request object.")
                client_id = message.get("client_id")
                if not isinstance(client_id, str) or len(client_id) != 32 or not all(c in "0123456789abcdef" for c in client_id):
                    raise RuntimeError("Invalid Oculus CLI request id.")
                request_generation = message.get("generation")
                if not isinstance(request_generation, int) or isinstance(request_generation, bool) or request_generation != generation[0]:
                    raise RuntimeError("Oculus CLI request belongs to a cancelled turn.")
                argv = message.get("argv")
                if not isinstance(argv, list) or len(argv) > 256 or not all(isinstance(x, str) for x in argv):
                    raise RuntimeError("Invalid Oculus CLI arguments.")
                answer = queue.Queue(maxsize=1)
                with pending_lock:
                    if len(pending) >= MAX_REQUESTS:
                        raise RuntimeError("Too many concurrent Oculus CLI requests.")
                    pending[identifier] = answer
                request = {"type": "oculus_cli_request", "id": identifier, "argv": argv, "generation": request_generation}
                if "stdin" in message:
                    if not isinstance(message["stdin"], str) or len(message["stdin"].encode("utf-8")) > 1024 * 1024:
                        raise RuntimeError("Invalid Oculus CLI input.")
                    request["stdin"] = message["stdin"]
                emit(request)
                deadline = time.monotonic() + 95
                while not stopped.is_set():
                    try:
                        result = answer.get(timeout=0.5)
                        result["client_id"] = client_id
                        write_response(slot, result)
                        return
                    except queue.Empty:
                        if time.monotonic() >= deadline:
                            raise RuntimeError("Oculus CLI request timed out.")
            except (OSError, ValueError, RuntimeError) as error:
                if client_id is not None:
                    write_response(slot, {"code": 1, "stdout": "", "stderr": str(error), "client_id": client_id})
            finally:
                with pending_lock:
                    pending.pop(identifier, None)

        def broker_loop(slot):
            descriptor = request_fds[slot]
            buffer = b""
            with selectors.DefaultSelector() as selector:
                selector.register(descriptor, selectors.EVENT_READ)
                while not stopped.is_set():
                    if not selector.select(0.2):
                        continue
                    try:
                        buffer += os.read(descriptor, 65536)
                    except BlockingIOError:
                        continue
                    # A process killed mid-request cannot keep the slot locked.
                    # A new request starts with a record separator (which JSON
                    # escapes inside string values), so even partial writes
                    # from a killed predecessor cannot corrupt the next call.
                    # discard the incomplete prefix at that next boundary.
                    while b"\n" in buffer:
                        line, buffer = buffer.split(b"\n", 1)
                        line = line.rsplit(b"\x1e", 1)[-1]
                        broker_request(slot, line + b"\n")
                    # Preserve a new incomplete frame when a large cancelled
                    # predecessor and its successor arrive in the same read.
                    # Size limits apply to the latest frame, not their sum.
                    separator = buffer.rfind(b"\x1e")
                    if separator > 0:
                        buffer = buffer[separator:]
                    if len(buffer) > 2 * 1024 * 1024:
                        buffer = b""

        for target in (input_loop, output_loop):
            threading.Thread(target=target, daemon=True).start()
        for slot in range(MAX_REQUESTS):
            threading.Thread(target=broker_loop, args=(slot,), daemon=True).start()
        try:
            while not stopped.wait(0.2) and child.poll() is None:
                if time.monotonic() - heartbeat[0] > HEARTBEAT_TIMEOUT:
                    stopped.set()
        finally:
            stopped.set()
            # Let a between-turn CLI flush its final transcript tail before
            # terminating the namespace. An active/stuck turn still gets only
            # this bounded grace period; detached workers cannot outlive it.
            def close_input():
                try:
                    child.stdin.close()
                except OSError:
                    pass
            # BufferedIO.close can wait for an in-flight pipe write. Never
            # let a child that stops reading stdin block namespace teardown.
            threading.Thread(target=close_input, daemon=True).start()
            try:
                child.wait(timeout=2)
            except (OSError, subprocess.TimeoutExpired):
                pass
            # bwrap is namespace PID 1's parent and owns its lifetime. Killing
            # it tears down every Linux descendant, even setsid/double-fork.
            try:
                if child.poll() is None:
                    os.killpg(child.pid, signal.SIGTERM)
                    child.wait(timeout=2)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=5)
            except ProcessLookupError:
                pass
        return child.returncode if child.returncode is not None and child.returncode >= 0 else 1


if __name__ == "__main__":
    try:
        mode = sys.argv[1]
        if mode == "probe":
            print(json.dumps(probe(), ensure_ascii=False))
        elif mode == "session":
            sys.exit(session())
        elif mode == "auth-login":
            sys.exit(auth_login())
        elif mode == "auth-status":
            info = runtime()
            sys.exit(subprocess.call([info["claude"], "auth", "status", "--json"],
                                    env=info["env"], cwd=info["home"]))
        elif mode == "models":
            sys.exit(models(sys.argv[2]))
        else:
            raise RuntimeError("Unknown Oculus bridge operation.")
    except Exception as error:
        print("Oculus WSL2 bridge: " + str(error), file=sys.stderr)
        sys.exit(1)

"""Real WSL2 containment/protocol tests without a model account or API call.

Run inside WSL2 as an ordinary user:
  python3 app/src-tauri/src/harness/test_wsl_bridge.py

Only the runtime executable discovery is replaced: the real supervisor,
bubblewrap, generated seccomp rules, FIFO broker and teardown all run.
"""
import errno
import json
import os
from pathlib import Path
import platform
import queue
import subprocess
import sys
import tempfile
import threading
import time
import unittest


MODULE = Path(__file__).with_name("wsl_bridge.py")
LAUNCHER = """
import importlib.util, json, sys
spec = importlib.util.spec_from_file_location('oculus_wsl_bridge', sys.argv[1])
bridge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge)
info = json.loads(sys.argv[2])
info['env'] = bridge.linux_environment(info['home'])
bridge.runtime = lambda: info
bridge.HEARTBEAT_TIMEOUT = float(sys.argv[3])
sys.exit(bridge.session())
"""

FAKE_CLAUDE = r'''
import ctypes, errno, fcntl, json, os, pathlib, signal, socket, subprocess, sys, time
root = pathlib.Path.cwd().parent
mode = sys.argv[1]
def emit(value):
    print(json.dumps(value, ensure_ascii=False), flush=True)
def attempt_write(path, value):
    try:
        pathlib.Path(path).write_text(value)
        return 0
    except OSError as error:
        return error.errno
if mode == 'echo':
    emit({'type': 'ready'})
    for line in sys.stdin:
        emit({'type': 'echo', 'value': json.loads(line)})
elif mode == 'broker':
    result = subprocess.run(['oculus', 'search', "量子 mechanics ' ; $(echo unsafe)", '--json'], capture_output=True, text=True)
    emit({'type': 'broker_result', 'code': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr})
    time.sleep(30)
elif mode == 'skills':
    emit({'type': 'skills', 'writes': [attempt_write(pathlib.Path.cwd() / path / 'SKILL.md', 'tampered')
                                     for path in ('skills', '.claude/skills', '.agents/skills')]})
    time.sleep(30)
elif mode == 'batch':
    batch = pathlib.Path.cwd() / 'batch 学生.json'
    batch.write_text('[{"title":"学生 task λ"}]', encoding='utf-8')
    for arguments in (
        ['--json', 'task', 'add', '--batch', str(batch)],
        ['task', '--json', 'add', '--batch=' + str(batch)],
    ):
        result = subprocess.run(['oculus'] + arguments, capture_output=True, text=True)
        emit({'type': 'batch_result', 'code': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr})
    time.sleep(30)
elif mode == 'inner_srt':
    settings = pathlib.Path.cwd() / 'inner-settings.json'
    settings.write_text(json.dumps({'filesystem': {'allowWrite': [str(pathlib.Path.cwd())], 'denyRead': [], 'denyWrite': []}, 'network': {'allowedDomains': [], 'deniedDomains': [], 'allowAllUnixSockets': False}}))
    inner = pathlib.Path.cwd() / 'inner-test.py'
    inner.write_text('import errno,json,socket,subprocess\ntry:\n socket.socket(socket.AF_UNIX,socket.SOCK_STREAM)\n raise RuntimeError("Unix sockets were not blocked")\nexcept OSError as error:\n assert error.errno==errno.EPERM,error\nr=subprocess.run(["oculus","status","--json"],capture_output=True,text=True)\nprint(json.dumps({"unix_blocked":True,"code":r.returncode,"stdout":r.stdout,"stderr":r.stderr}))\n')
    result = subprocess.run(['/home/oculus/.bun/bin/bun', '/home/oculus/.bun/install/global/node_modules/@anthropic-ai/sandbox-runtime/dist/cli.js', '--settings', str(settings), '--', sys.executable, str(inner)], capture_output=True, text=True, timeout=25)
    emit({'type': 'inner_result', 'code': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr, 'missing_lectures': not (root / 'lectures').exists()})
    time.sleep(30)
elif mode == 'concurrent':
    jobs = [subprocess.Popen(['oculus', 'search', 'concurrent-' + str(index)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) for index in range(4)]
    for index, job in enumerate(jobs):
        stdout, stderr = job.communicate(timeout=20)
        emit({'type': 'concurrent_result', 'index': index, 'code': job.returncode, 'stdout': stdout, 'stderr': stderr})
    time.sleep(30)
elif mode == 'large_batch':
    content = json.dumps([{'title': '学生 λ' * 40000}], ensure_ascii=False)
    result = subprocess.run(['oculus', 'task', 'add', '--batch', '-'], input=content, capture_output=True, text=True, timeout=20)
    emit({'type': 'large_result', 'bytes': len(content.encode()), 'code': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr})
    time.sleep(30)
elif mode in ('reuse_partial', 'reuse_malformed', 'reuse_large_partial'):
    # Simulate a killed caller leaving a partial frame and stale response.
    with open('/run/oculus/slot-0.lock', 'rb') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        request = os.open('/run/oculus/request-0.fifo', os.O_WRONLY | os.O_NONBLOCK)
        if mode == 'reuse_malformed':
            prefix = b'\x1enull\n\x1e[]\n'
        elif mode == 'reuse_large_partial':
            prefix = b'\x1e{"client_id":"cancelled-partial' + b'x' * (2 * 1024 * 1024 - 1024)
        else:
            prefix = b'\x1e{"client_id":"cancelled-partial'
        position = 0
        while position < len(prefix):
            try: position += os.write(request, prefix[position:position + 4096])
            except BlockingIOError: time.sleep(0.002)
        os.close(request)
        time.sleep(0.1)
        response = os.open('/run/oculus/response-0.fifo', os.O_RDWR | os.O_NONBLOCK)
        os.write(response, b'\x1e{"client_id":"stale","stdout":"WRONG"}\n\x1e{"partial":')
        os.close(response)
    if mode == 'reuse_large_partial':
        payload = json.dumps([{'title': '学生 λ' * 40000}], ensure_ascii=False)
        result = subprocess.run(['oculus', 'task', 'add', '--batch', '-'], input=payload, capture_output=True, text=True, timeout=20)
    else:
        result = subprocess.run(['oculus', 'search', 'after-cancel'], capture_output=True, text=True, timeout=20)
    emit({'type': 'reuse_result', 'code': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr})
    time.sleep(30)
elif mode == 'malformed_response':
    caller = subprocess.Popen(['oculus', 'search', 'malformed-response'], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    time.sleep(0.2)
    descriptor = os.open('/run/oculus/response-0.fifo', os.O_WRONLY | os.O_NONBLOCK)
    os.write(descriptor, b'\x1enull\n\x1e[]\n')
    os.close(descriptor)
    emit({'type': 'response_injected'})
    stdout, stderr = caller.communicate(timeout=20)
    emit({'type': 'reuse_result', 'code': caller.returncode, 'stdout': stdout, 'stderr': stderr})
    time.sleep(30)
elif mode == 'cancel_inflight':
    first = subprocess.Popen(['oculus', 'search', 'cancel-inflight'], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    emit({'type': 'cancel_ready'})
    command = json.loads(sys.stdin.readline())
    assert command['type'] == 'user'
    first.kill()
    first.wait()
    emit({'type': 'cancelled'})
    result = subprocess.run(['oculus', 'search', 'after-inflight'], capture_output=True, text=True, timeout=20)
    emit({'type': 'reuse_result', 'code': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr})
    time.sleep(30)
elif mode == 'generation_barrier':
    lock = open('/run/oculus/slot-0.lock', 'rb')
    fcntl.flock(lock, fcntl.LOCK_EX)
    response = os.open('/run/oculus/response-0.fifo', os.O_RDWR | os.O_NONBLOCK)
    request = os.open('/run/oculus/request-0.fifo', os.O_WRONLY | os.O_NONBLOCK)
    for identifier, arguments in (('1' * 32, ['search', 'first']), ('2' * 32, ['task', 'add', 'stale-must-not-invoke'])):
        frame = b'\x1e' + json.dumps({'client_id': identifier, 'generation': 0, 'argv': arguments}).encode() + b'\n'
        os.write(request, frame)
    os.close(request)
    emit({'type': 'generation_queued'})
    inputs = [json.loads(sys.stdin.readline()), json.loads(sys.stdin.readline())]
    emit({'type': 'generation_inputs', 'kinds': [message['type'] for message in inputs], 'private_leaked': any('oculus_generation' in message for message in inputs)})
    buffer = b''
    replies = []
    deadline = time.monotonic() + 20
    while len(replies) < 2 and time.monotonic() < deadline:
        try: buffer += os.read(response, 65536)
        except BlockingIOError: time.sleep(0.01)
        while b'\n' in buffer:
            line, buffer = buffer.split(b'\n', 1)
            replies.append(json.loads(line.rsplit(b'\x1e', 1)[-1]))
    os.close(response)
    lock.close()
    emit({'type': 'generation_replies', 'replies': replies})
    result = subprocess.run(['oculus', 'status'], capture_output=True, text=True, timeout=20)
    emit({'type': 'reuse_result', 'code': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr})
    time.sleep(30)
elif mode == 'containment':
    (pathlib.Path.cwd() / 'outside-link').symlink_to(root / 'materials' / 'sentinel.txt')
    result = {
        'type': 'containment',
        'allowed_write': attempt_write(pathlib.Path.cwd() / '允许.txt', 'inside only'),
        'blocked_write': attempt_write(root / 'materials' / 'sentinel.txt', 'must not happen'),
        'blocked_symlink_write': attempt_write(pathlib.Path.cwd() / 'outside-link', 'must not happen'),
        'library_read': (root / 'materials' / 'sentinel.txt').read_text(),
        'wsl_interop_env': os.environ.get('WSL_INTEROP'),
        'wsl_run_visible': pathlib.Path('/run/WSL').exists(),
        'windows_drive_visible': pathlib.Path('/mnt/c/Windows').exists(),
        'windows_path_imported': '/mnt/c/' in os.environ['PATH'],
        'resolv_conf': pathlib.Path('/etc/resolv.conf').read_text(),
    }
    try:
        connection = socket.socket(socket.AF_VSOCK, socket.SOCK_STREAM)
        connection.close()
        result['vsock_errno'] = 0
    except OSError as error:
        result['vsock_errno'] = error.errno
    library = ctypes.CDLL(None, use_errno=True)
    ctypes.set_errno(0)
    result['ptrace_return'] = library.ptrace(0, 0, None, None)
    result['ptrace_errno'] = ctypes.get_errno()
    try:
        process = subprocess.run(['/mnt/c/Windows/System32/cmd.exe', '/c', 'echo OCULUS_INTEROP_MUST_BE_BLOCKED'], capture_output=True, timeout=5)
        result['windows_return'] = process.returncode
        result['windows_errno'] = 0
    except OSError as error:
        result['windows_errno'] = error.errno
    emit(result)
    time.sleep(30)
elif mode == 'descendant':
    # This child deliberately detaches and ignores graceful termination.
    # The namespace owner must still reap it when Windows disappears.
    marker = str(pathlib.Path.cwd() / 'descendant-heartbeat')
    child_code = 'import pathlib,signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); p=pathlib.Path(' + repr(marker) + ');\nwhile True: p.write_text(str(time.monotonic_ns())); time.sleep(0.04)'
    subprocess.Popen([sys.executable, '-u', '-c', child_code], start_new_session=True, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    emit({'type': 'descendant_ready'})
    time.sleep(60)
else:
    raise ValueError(mode)
'''


class Session:
    def __init__(self, test, mode, cwd=None, heartbeat_timeout=2):
        info = {
            "home": str(test.home),
            "config": str(test.home / ".claude"),
            "claude": os.path.realpath(sys.executable),
            "version": "test Python executable",
        }
        self.output = queue.Queue()
        self.process = subprocess.Popen(
            [sys.executable, "-u", "-c", LAUNCHER, str(MODULE), json.dumps(info), str(heartbeat_timeout)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, encoding="utf-8", bufsize=1,
        )
        self.errors = []
        def stdout():
            for line in self.process.stdout:
                try:
                    self.output.put(json.loads(line))
                except ValueError:
                    self.output.put({"invalid_output": line})
            self.output.put(None)
        def stderr():
            self.errors.extend(self.process.stderr.readlines())
        threading.Thread(target=stdout, daemon=True).start()
        threading.Thread(target=stderr, daemon=True).start()
        self.send({
            "library": str(test.library),
            "cwd": str(cwd or test.agents),
            "args": ["-u", str(test.agents / "fake_claude.py"), mode],
        })
        test.addCleanup(self.close)

    def send(self, value):
        self.process.stdin.write(json.dumps(value, ensure_ascii=False) + "\n")
        self.process.stdin.flush()

    def receive(self, timeout=8):
        value = self.output.get(timeout=timeout)
        if value is None:
            raise AssertionError("Supervisor ended before expected output: " + "".join(self.errors))
        return value

    def end_input(self):
        if self.process.stdin and not self.process.stdin.closed:
            self.process.stdin.close()

    def close(self):
        self.end_input()
        try:
            self.process.wait(timeout=6)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait(timeout=3)
        self.process.stdout.close()
        self.process.stderr.close()


@unittest.skipUnless(os.name == "posix" and "wsl2" in platform.release().lower(), "Requires WSL2")
class WslBridgeTests(unittest.TestCase):
    def setUp(self):
        self.assertNotEqual(os.getuid(), 0, "Run bridge tests as an ordinary Linux user")
        # Use the Linux home rather than /tmp, which the sandbox deliberately masks.
        self.temporary = tempfile.TemporaryDirectory(prefix="oculus-wsl-test-", dir=Path.home())
        self.addCleanup(self.temporary.cleanup)
        root = Path(self.temporary.name)
        self.home = root / "test home 学生"
        self.home.mkdir()
        self.library = root / "library 量子 (Windows)"
        self.agents = self.library / "agents"
        self.agents.mkdir(parents=True)
        (self.library / "materials").mkdir()
        (self.library / "materials" / "sentinel.txt").write_text("original readonly material")
        (self.agents / "fake_claude.py").write_text(FAKE_CLAUDE)

    def test_stream_forwards_unicode_user_and_control_messages(self):
        session = Session(self, "echo")
        self.assertEqual(session.receive(), {"type": "ready"})
        for message in (
            {"type": "user", "message": {"role": "user", "content": "学生 says 'hello' ☃"}},
            {"type": "control_request", "request_id": "cancel-123", "request": {"subtype": "interrupt"}},
        ):
            session.send(message)
            self.assertEqual(session.receive(), {"type": "echo", "value": message})

    def test_cli_broker_preserves_arguments_and_exit_status(self):
        session = Session(self, "broker")
        request = session.receive()
        self.assertEqual(request["type"], "oculus_cli_request")
        self.assertEqual(request["argv"], ["search", "量子 mechanics ' ; $(echo unsafe)", "--json"])
        session.send({"type": "oculus_cli_response", "id": request["id"], "code": 7,
                      "stdout": "retrieval 学生\n", "stderr": "diagnostic λ\n"})
        self.assertEqual(session.receive(), {"type": "broker_result", "code": 7,
                                            "stdout": "retrieval 学生\n", "stderr": "diagnostic λ\n"})

    def test_outer_sandbox_restricts_writes_and_windows_escape(self):
        session = Session(self, "containment")
        result = session.receive()
        self.assertEqual(result["type"], "containment")
        self.assertEqual(result["allowed_write"], 0)
        self.assertIn(result["blocked_write"], (errno.EROFS, errno.EACCES, errno.EPERM))
        self.assertIn(result["blocked_symlink_write"], (errno.EROFS, errno.EACCES, errno.EPERM))
        self.assertEqual(result["library_read"], "original readonly material")
        self.assertEqual((self.library / "materials" / "sentinel.txt").read_text(), "original readonly material")
        self.assertEqual((self.agents / "允许.txt").read_text(), "inside only")
        self.assertIsNone(result["wsl_interop_env"])
        self.assertFalse(result["wsl_run_visible"])
        self.assertFalse(result["windows_drive_visible"])
        self.assertFalse(result["windows_path_imported"])
        self.assertTrue(any(line.startswith("nameserver ") for line in result["resolv_conf"].splitlines()))
        self.assertEqual(result["vsock_errno"], errno.EPERM)
        self.assertEqual(result["ptrace_return"], -1)
        self.assertEqual(result["ptrace_errno"], errno.EPERM)
        self.assertIn(result["windows_errno"], (errno.ENOEXEC, errno.EACCES, errno.EPERM, errno.ENOENT))

    def test_batch_broker_reads_unicode_files_inside_sandbox(self):
        session = Session(self, "batch")
        for expected in (
            ["--json", "task", "add", "--batch", "-"],
            ["task", "--json", "add", "--batch=-"],
        ):
            request = session.receive()
            self.assertEqual(request["type"], "oculus_cli_request")
            self.assertEqual(request["argv"], expected)
            self.assertEqual(request["stdin"], '[{"title":"学生 task λ"}]')
            session.send({"type": "oculus_cli_response", "id": request["id"], "code": 0,
                          "stdout": "created 学生\n", "stderr": ""})
            self.assertEqual(session.receive(), {"type": "batch_result", "code": 0,
                                                "stdout": "created 学生\n", "stderr": ""})

    def answer(self, session, request, stdout="correct response\n"):
        self.assertEqual(request["type"], "oculus_cli_request")
        session.send({"type": "oculus_cli_response", "id": request["id"], "code": 0,
                      "stdout": stdout, "stderr": ""})

    def test_actual_inner_srt_keeps_unix_sockets_blocked_and_fifo_working(self):
        session = Session(self, "inner_srt", heartbeat_timeout=30)
        request = session.receive(timeout=25)
        self.assertEqual(request.get("type"), "oculus_cli_request", request)
        self.assertEqual(request["argv"], ["status", "--json"])
        self.answer(session, request, "INNER_FIFO_OK\n")
        result = session.receive(timeout=15)
        self.assertEqual(result["type"], "inner_result")
        self.assertEqual(result["code"], 0, result)
        self.assertTrue(result["missing_lectures"])
        inner = json.loads(result["stdout"])
        self.assertEqual(inner, {"unix_blocked": True, "code": 0, "stdout": "INNER_FIFO_OK\n", "stderr": ""})

    def test_four_concurrent_requests_keep_responses_separate(self):
        session = Session(self, "concurrent", heartbeat_timeout=15)
        requests = [session.receive() for _ in range(4)]
        self.assertEqual({r["argv"][1] for r in requests}, {"concurrent-" + str(i) for i in range(4)})
        for request in reversed(requests):
            self.answer(session, request, request["argv"][1])
        results = [session.receive() for _ in range(4)]
        for result in results:
            self.assertEqual(result, {"type": "concurrent_result", "index": result["index"], "code": 0,
                                     "stdout": "concurrent-" + str(result["index"]), "stderr": ""})

    def test_large_unicode_stdin_and_response_cross_fifo_buffers(self):
        session = Session(self, "large_batch", heartbeat_timeout=15)
        request = session.receive()
        self.assertEqual(request["argv"], ["task", "add", "--batch", "-"])
        self.assertEqual(json.loads(request["stdin"]), [{"title": "学生 λ" * 40000}])
        response = "result 学生 λ\n" * 18000
        self.answer(session, request, response)
        result = session.receive()
        self.assertEqual(result["type"], "large_result")
        self.assertGreater(result["bytes"], 300000)
        self.assertEqual(result["code"], 0)
        self.assertEqual(result["stdout"], response)
        self.assertEqual(result["stderr"], "")

    def test_partial_cancelled_frames_do_not_poison_reused_slot(self):
        session = Session(self, "reuse_partial", heartbeat_timeout=15)
        request = session.receive()
        self.assertEqual(request["argv"], ["search", "after-cancel"])
        self.answer(session, request)
        self.assertEqual(session.receive(), {"type": "reuse_result", "code": 0,
                                            "stdout": "correct response\n", "stderr": ""})

    def test_malformed_json_objects_do_not_kill_broker_slot(self):
        session = Session(self, "reuse_malformed", heartbeat_timeout=15)
        request = session.receive()
        self.assertEqual(request["argv"], ["search", "after-cancel"])
        self.answer(session, request)
        self.assertEqual(session.receive(), {"type": "reuse_result", "code": 0,
                                            "stdout": "correct response\n", "stderr": ""})

    def test_large_partial_frame_resynchronizes_before_limit_check(self):
        session = Session(self, "reuse_large_partial", heartbeat_timeout=15)
        request = session.receive()
        self.assertEqual(request["argv"], ["task", "add", "--batch", "-"])
        self.assertEqual(json.loads(request["stdin"]), [{"title": "学生 λ" * 40000}])
        self.answer(session, request)
        self.assertEqual(session.receive(), {"type": "reuse_result", "code": 0,
                                            "stdout": "correct response\n", "stderr": ""})

    def test_malformed_responses_are_ignored_until_matching_response(self):
        session = Session(self, "malformed_response", heartbeat_timeout=15)
        messages = [session.receive(), session.receive()]
        self.assertTrue(any(message.get("type") == "response_injected" for message in messages))
        request = next(message for message in messages if message.get("type") == "oculus_cli_request")
        self.assertEqual(request["argv"], ["search", "malformed-response"])
        self.answer(session, request)
        self.assertEqual(session.receive(), {"type": "reuse_result", "code": 0,
                                            "stdout": "correct response\n", "stderr": ""})

    def test_cancelled_inflight_response_cannot_reach_next_client(self):
        session = Session(self, "cancel_inflight", heartbeat_timeout=15)
        messages = [session.receive(), session.receive()]
        self.assertTrue(any(message.get("type") == "cancel_ready" for message in messages))
        old = next(message for message in messages if message.get("type") == "oculus_cli_request")
        self.assertEqual(old["argv"], ["search", "cancel-inflight"])
        session.send({"type": "user", "message": {"content": "cancel"}})
        self.assertEqual(session.receive(), {"type": "cancelled"})
        self.answer(session, old, "OLD_RESPONSE_MUST_NOT_REACH_NEW_CLIENT")
        fresh = session.receive()
        self.assertEqual(fresh["argv"], ["search", "after-inflight"])
        self.answer(session, fresh)
        self.assertEqual(session.receive(), {"type": "reuse_result", "code": 0,
                                            "stdout": "correct response\n", "stderr": ""})

    def test_queued_previous_turn_request_is_rejected_before_host_dispatch(self):
        session = Session(self, "generation_barrier", heartbeat_timeout=20)
        first_messages = [session.receive(), session.receive()]
        self.assertTrue(any(message.get("type") == "generation_queued" for message in first_messages))
        first = next(message for message in first_messages if message.get("type") == "oculus_cli_request")
        self.assertEqual(first["argv"], ["search", "first"])
        self.assertEqual(first["generation"], 0)
        session.send({"type": "control_request", "oculus_generation": 1, "request": {"subtype": "interrupt"}})
        session.send({"type": "user", "oculus_generation": 2, "message": {"content": "next turn"}})
        self.assertEqual(session.receive(), {"type": "generation_inputs", "kinds": ["control_request", "user"], "private_leaked": False})
        self.answer(session, first)
        replies = session.receive()
        self.assertEqual(replies["type"], "generation_replies", replies)
        self.assertEqual(len(replies["replies"]), 2)
        cancelled = next(reply for reply in replies["replies"] if reply["client_id"] == "2" * 32)
        self.assertEqual(cancelled["code"], 1)
        self.assertIn("cancelled turn", cancelled["stderr"])
        fresh = session.receive()
        self.assertEqual(fresh["argv"], ["status"])
        self.assertEqual(fresh["generation"], 2)
        self.answer(session, fresh)
        self.assertEqual(session.receive(), {"type": "reuse_result", "code": 0,
                                            "stdout": "correct response\n", "stderr": ""})

    def descendant_session(self):
        session = Session(self, "descendant")
        self.assertEqual(session.receive(), {"type": "descendant_ready"})
        marker = self.agents / "descendant-heartbeat"
        deadline = time.monotonic() + 3
        while not marker.exists() and time.monotonic() < deadline:
            time.sleep(0.02)
        self.assertTrue(marker.exists(), "Detached child never started")
        initial = marker.read_text()
        time.sleep(0.15)
        self.assertNotEqual(initial, marker.read_text(), "Detached child did not write its heartbeat")
        return session, marker

    def assert_descendant_stopped(self, session, marker):
        session.process.wait(timeout=7)
        # Allow namespace teardown to settle before proving no detached writes.
        time.sleep(0.2)
        stopped = marker.read_text()
        time.sleep(0.3)
        self.assertEqual(marker.read_text(), stopped, "Detached grandchild survived supervisor shutdown")

    def test_stdin_eof_kills_detached_descendants(self):
        session, marker = self.descendant_session()
        session.end_input()
        self.assert_descendant_stopped(session, marker)

    def test_missing_owner_heartbeat_kills_detached_descendants(self):
        session, marker = self.descendant_session()
        self.assert_descendant_stopped(session, marker)

    def test_heartbeats_keep_session_alive_until_eof(self):
        session, marker = self.descendant_session()
        for _ in range(6):
            session.send({"type": "oculus_ping"})
            time.sleep(0.45)
            self.assertIsNone(session.process.poll())
        session.end_input()
        self.assert_descendant_stopped(session, marker)

    def test_invalid_working_directory_fails_before_child_start(self):
        other = self.library / "materials"
        session = Session(self, "echo", cwd=other)
        code = session.process.wait(timeout=7)
        self.assertNotEqual(code, 0)
        self.assertFalse((self.agents / "允许.txt").exists())

    def test_generated_skills_are_readonly_inside_the_writable_agent_folder(self):
        for relative in ("skills", ".claude/skills", ".agents/skills"):
            folder = self.agents / relative
            folder.mkdir(parents=True)
            (folder / "SKILL.md").write_text("generated")
        session = Session(self, "skills")
        result = session.receive()
        self.assertEqual(result["type"], "skills")
        for error in result["writes"]:
            self.assertIn(error, (errno.EROFS, errno.EACCES, errno.EPERM))
        for relative in ("skills", ".claude/skills", ".agents/skills"):
            self.assertEqual((self.agents / relative / "SKILL.md").read_text(), "generated")

    def test_sign_in_preserves_code_and_cancels_on_owner_eof(self):
        fake = self.home / "fake-auth"
        fake.write_text("#!/usr/bin/python3\n"
                        "import json,os,sys,time\n"
                        "assert sys.argv[1:]==['auth','login']\n"
                        "print(json.dumps({'ready':os.getpid()}),flush=True)\n"
                        "code=sys.stdin.readline().strip()\n"
                        "print(json.dumps({'code':code},ensure_ascii=False),flush=True)\n")
        fake.chmod(0o700)
        launcher = LAUNCHER.replace("sys.exit(bridge.session())", "sys.exit(bridge.auth_login(info))")
        info = json.dumps({"home": str(self.home), "claude": str(fake)})
        for send_code in (True, False):
            process = subprocess.Popen([sys.executable, "-u", "-c", launcher,
                                        str(MODULE), info, "15"],
                                       stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                       stderr=subprocess.PIPE, text=True)
            try:
                pid = json.loads(process.stdout.readline())["ready"]
                if send_code:
                    process.stdin.write("code 学生 λ\n")
                    process.stdin.flush()
                    self.assertEqual(json.loads(process.stdout.readline()), {"code": "code 学生 λ"})
                    self.assertEqual(process.wait(timeout=5), 0)
                else:
                    process.stdin.close()
                    self.assertNotEqual(process.wait(timeout=5), 0)
                    with self.assertRaises(ProcessLookupError):
                        os.kill(pid, 0)
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=5)
                for pipe in (process.stdin, process.stdout, process.stderr):
                    if not pipe.closed:
                        pipe.close()

    def model_catalogue_process(self, answer=True, timeout=3):
        fake = self.home / "fake-models"
        pidfile = self.home / "catalogue.pid"
        fake.write_text("#!/usr/bin/python3\n"
                        "import json,os,pathlib,sys,time\n"
                        "args=sys.argv[1:]\n"
                        "assert args==['-p','--input-format','stream-json','--output-format','stream-json','--verbose','--no-session-persistence','--strict-mcp-config','--settings','{\"disableAllHooks\":true}'],args\n"
                        "assert 'ANTHROPIC_API_KEY' not in os.environ\n"
                        f"pathlib.Path({str(pidfile)!r}).write_text(str(os.getpid()))\n"
                        "request=json.loads(sys.stdin.readline())\n"
                        "assert request=={'type':'control_request','request_id':'oculus-models','request':{'subtype':'initialize'}}\n"
                        "print('not a JSON protocol line',flush=True)\n"
                        + ("print(json.dumps({'type':'control_response','response':{'request_id':'oculus-models','subtype':'success','response':{'models':[{'value':'sonnet','resolvedModel':'claude-test','displayName':'Claude test','supportedEffortLevels':['low','high']}]}}}),flush=True)\n" if answer else "")
                        + "time.sleep(30)\n")
        fake.chmod(0o700)
        launcher = LAUNCHER.replace("sys.exit(bridge.session())",
                                    "sys.exit(bridge.models(sys.argv[4], info, timeout=float(sys.argv[3])))")
        process = subprocess.Popen([sys.executable, "-u", "-c", launcher, str(MODULE),
                                    json.dumps({"home": str(self.home), "claude": str(fake)}),
                                    str(timeout), str(self.agents)], stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

        def cleanup():
            if process.poll() is None:
                process.stdin.close()
                process.wait(timeout=5)
            for pipe in (process.stdin, process.stdout, process.stderr):
                if not pipe.closed:
                    pipe.close()

        self.addCleanup(cleanup)
        return process, pidfile

    def test_catalogue_uses_only_initialize_and_reaps_the_idle_cli(self):
        process, pidfile = self.model_catalogue_process()
        process.stdin.write(json.dumps({"type": "control_request", "request_id": "oculus-models",
                                        "request": {"subtype": "initialize"}}) + "\n")
        process.stdin.flush()
        answer = json.loads(process.stdout.readline())
        self.assertEqual(answer["response"]["response"]["models"][0]["resolvedModel"], "claude-test")
        self.assertEqual(process.wait(timeout=5), 0)
        with self.assertRaises(ProcessLookupError):
            os.kill(int(pidfile.read_text()), 0)

    def test_catalogue_rejects_a_user_turn_before_starting_claude(self):
        process, pidfile = self.model_catalogue_process()
        process.stdin.write(json.dumps({"type": "user", "message": {"content": "never send"}}) + "\n")
        process.stdin.flush()
        self.assertNotEqual(process.wait(timeout=5), 0)
        self.assertFalse(pidfile.exists())
        self.assertIn("only an initialize request", process.stderr.read())

    def test_catalogue_deadline_and_owner_eof_kill_the_linux_child(self):
        for owner_eof in (True, False):
            process, pidfile = self.model_catalogue_process(answer=False, timeout=1)
            pidfile.unlink(missing_ok=True)
            process.stdin.write(json.dumps({"type": "control_request", "request_id": "oculus-models",
                                            "request": {"subtype": "initialize"}}) + "\n")
            process.stdin.flush()
            deadline = time.monotonic() + 3
            while not pidfile.exists() and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertTrue(pidfile.exists())
            pid = int(pidfile.read_text())
            if owner_eof:
                process.stdin.close()
            self.assertNotEqual(process.wait(timeout=5), 0)
            with self.assertRaises(ProcessLookupError):
                os.kill(pid, 0)
            if not owner_eof:
                self.assertIn("Linux deadline", process.stderr.read())


if __name__ == "__main__":
    unittest.main(verbosity=2)

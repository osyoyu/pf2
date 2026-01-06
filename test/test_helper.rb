# frozen_string_literal: true

def sample_now
  Process.kill("SIGPROF", Process.pid)
end


def system_with_timeout(command, timeout)
  pid = spawn(command, pgroup: true)
  pgid = Process.getpgid(pid)
  wait_thread = Process.detach(pid)
  if wait_thread.nil?
    # The command has already exited
    return Process.last_status
  end
  if wait_thread.join(timeout) == nil
    # The command didn't exit by the timeout
    Process.kill(:KILL, -pgid)
  end
  wait_thread.value
end

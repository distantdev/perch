> **Note:** The shipped product name is **Perch** (`perch` CLI); this spec retains the original Portmaster naming in places.

# Software Specification: Project Portmaster (Dev Server Monitor & Controller)

**Date:** May 26, 2026  

**Status:** Draft / Technical Proposal  

**Target Architecture:** Cross-Platform (Windows, macOS, Linux)  

---

## 1. Executive Summary

### 1.1 Problem Statement

Modern developers frequently run multiple local web servers simultaneously across varied tech stacks (e.g., Node.js frontend, .NET Core API, Python background workers, Docker-managed databases). These servers are spun up via distinct toolchains `npm run dev`, `dotnet run`, `docker-compose up`, etc.) across multiple terminal tabs, multiplexers, or IDEs. 

This ecosystem creates several operational friction points:

1. **Orphaned Processes:** Closing a terminal window or IDE abruptly often leaves behind ghost processes binding to ports, preventing subsequent spin-ups `EADDRINUSE`).

2. **Resource Exhaustion:** Background development servers consume substantial CPU/Memory cycles even when idle or not actively under test.

3. **Discovery Friction:** Tracking down which repository is running on which port requires manual shell intervention `lsof`, `netstat`).

### 1.2 Proposed Solution

`portmaster` is a lightweight, zero-dependency, compiled CLI utility that ambiently tracks, inspects, and manages active development servers without altering their setup scripts, wrappers, or runtime parameters. It treats the local operating system network socket table as the single source of truth, deriving process ownership, context, and directory roots dynamically.

---

## 2. Core Philosophy & Design Constraints

* **Ambient Inspection:** No background daemon, agent, or client code is required to run permanently. The utility evaluates system state on-demand.

* **Non-Invasive Management:** The tool does not alter process instantiation or environment configuration. It exercises management through standard OS signals `SIGTERM`, `SIGSTOP`, `SIGCONT`).

* **High Efficiency:** Low resource usage, near-instant launch times (\<50ms execution window), and self-contained deployment (single binary).

* **Keyboard-Centric Interaction:** A fluid Terminal User Interface (TUI) optimized for vim/emacs navigation keys, quick filters, and structural hotkeys.

---

## 3. System Architecture & Mechanics

### 3.1 Detection Architecture

The application runs a 3-step pipeline to discover and hydrate development server information:

```

\[ Network Socket Scan \] ──> \[ PID Extraction \] ──> \[ Process Context Hydration \]

   (Reads local TCP/UDP       (Maps bound ports       (Inspects CMD, CWD, and 

     binding tables)            to active PIDs)         parent process trees)

```

1. **Network Socket Scan:** Queries the OS network stack for active `TCP` and `UDP` listeners bound to local loopback addresses `127.0.0.1`, `::1`) or wildcard interfaces `0.0.0.0`, `::`).

2. **PID Extraction:** Maps the isolated network socket directly to its owning Process Identifier (PID).

3. **Process Context Hydration:** For each extracted PID, the engine queries the OS kernel process table to collect:

   * **Executable Image Name:** e.g., `node`, `dotnet`, `python3`, `docker-proxy`.

   * **Current Working Directory (CWD):** The absolute path where the process was spawned (used to deduce the repository/project root).

   * **Full Command Line Invocation:** The exact arguments (e.g., `run dev`, `bin/rails server`) to categorize the framework type.

   * **Process Resource Consumption:** Current CPU usage percentage and Resident Set Size (RSS) memory consumption.

   * **Process Age:** Total wall-clock elapsed uptime.

### 3.2 Platform-Specific Implementations

To maintain zero runtime dependencies, the system leverages native OS facilities directly or interfaces with core system libraries via standard cross-platform platform abstraction wrappers:

#### 3.2.1 Linux Architecture

* **Sockets:** Parses `/proc/net/tcp` and `/proc/net/tcp6` interface structures directly.

* **Metadata:** Inspects `/proc/[PID]/cwd` (symbolic link to directory), `/proc/[PID]/cmdline` (null-terminated arguments), and `/proc/[PID]/stat` (process metrics).

#### 3.2.2 macOS (Darwin) Architecture

* **Sockets & PIDs:** Employs `sysctl` routines alongside system-level `proc_pidinfo` definitions (mirroring `lsof` mechanisms without execution overhead).

* **Metadata:** Evaluates `proc_pidfdinfo` and `proc_pidpath` system APIs.

#### 3.2.3 Windows Architecture

* **Sockets & PIDs:** Invokes `GetExtendedTcpTable` from `IPHLPAPI.DLL` to capture socket-to-PID bindings.

* **Metadata:** Leverages `OpenProcess` and queries via `QueryFullProcessImageNameW` and `NtQueryInformationProcess` to safely evaluate command lines and working environments.

---

## 4. Functional Requirements

### 4.1 Command Line Interface (CLI) Modes

The utility operates in two modes: **Standard/Scripting Output Mode** and **Interactive TUI Mode**.

#### 4.1.1 Scripting Mode Commands

* `pm list` / `pm ls`: Emits a plain text table or machine-readable format of active servers.

  * `--format json`: Outputs structured JSON data for integration with shell wrappers or custom scripts.

* `pm kill <port|pid>`: Issues an immediate termination command to the process owning the designated port/PID.

  * `-f`, `--force`: escalates execution directly to uncatchable kill signals `SIGKILL` / `TerminateProcess`).

* `pm pause <port|pid>`: Halts execution loops on systems supporting execution control loops.

* `pm resume <port|pid>`: Re-activates a previously halted development server process.

#### 4.1.2 Interactive TUI Dashboard

Triggered by running `pm` or `pm ui` without arguments.

```text

+--------------------------------------------------------------------------------+

| PORTMASTER v1.0.0                       [Filters: node        ] [Sort: Port]   |

+--------------------------------------------------------------------------------+

|  PORT   TYPE      PID    PROJECT DIRECTORY        CPU     MEM      UPTIME      |

| ------------------------------------------------------------------------------ |

|  3000   Node      45120  ~/dev/work/web-app       1.2%    142MB    02:14:05    |

|  5001   .NET      45211  ~/dev/work/api-service   0.1%    89MB     02:12:30    |

|  8080   Docker    1102   ~/dev/infra/local-db     0.0%    45MB     1d 04h      |

| >8000   Python    46890  ~/dev/sandbox/ai-script  94.2%   1.2GB    00:04:12    |

|                                                                                |

|                                                                                |

|                                                                                |

+--------------------------------------------------------------------------------+

| [K] Kill Process   [F] Force Kill   [P] Freeze/Pause   [C] Continue   [Q] Quit |

+--------------------------------------------------------------------------------+

```

### 4.2 Lifecycle & Signal Management

The app uses standard OS signal structures to execute state operations safely:

| Feature Target | POSIX Standard Signal (macOS/Linux) | Windows API Vector |

| :--- | :--- | :--- |

| **Graceful Shutdown** | `SIGTERM` (15) | `WM_CLOSE` / Event Signal injection |

| **Enforced Kill** | `SIGKILL` (9) | `TerminateProcess` |

| **Process Freeze** | `SIGSTOP` (19) | `NtSuspendProcess` (Undocumented Kernel API) |

| **Process Thaw** | `SIGCONT` (18) | `NtResumeProcess` (Undocumented Kernel API) |

---

## 5. Technology Stack Selection

### 5.1 Compilation Strategy: Go vs. Rust vs. .NET Native AOT

To guarantee optimal deployment characteristics, three paths meet the structural prerequisite:

1. **Rust (with `ratatui` + `sysinfo`):** High type safety, predictable cross-compilation matrix, direct integration with native POSIX/Win32 headers via FFI, tiny footprint (~1.5MB executable binary).

2. **Go (with `bubbletea` + `shirou/gopsutil`):** Fast development loop, highly concurrent runtime model built for lightweight asynchronous updates, marginally larger binaries (~6MB).

3. **C# .NET 10 (with Native AOT + `Spectre.Console`):** Exceptional text layout systems, highly ergonomic development experience for modern systems, fast execution loops via native compilation blocks (~5MB binary).

*Recommendation:* **Rust** offers the most deterministic memory tracking boundaries and clean execution models when manipulating raw POSIX or low-level Win32 system APIs `NtSuspendProcess`), making it the premier engine platform for this spec.

---

## 6. Security & Edge Case Mitigation

### 6.1 Multi-Container Mapping (Docker Proxies)

* **Edge Case:** Sockets mapped via Docker containers bind directly to host ports via `docker-proxy` or `vpnkit`. Inspecting the PID directly yields the Docker proxy rather than the active application context inside the container.

* **Mitigation Engine:** When `portmaster` detects an owning executable named `docker-proxy`, it initiates a contextual fallback tracking lookup. It queries `docker ps` or inspects local container socket tables to extract the container image name, internal process paths, and volumes to resolve true contextual identification.

### 6.2 Port Sweeps and False Positives

* **Edge Case:** Standard system infrastructure tasks, long-lived IDE communication listeners, or browser debugging sockets (e.g., Chrome DevTools on port 9222) pollute the dashboard interface.

* **Mitigation Engine:** Maintain a persistent, customizable local configuration file `~/.config/portmaster/config.toml`) specifying an exclude/ignore list of standard binary profiles or known infrastructure daemon ports (e.g., SSH on 22, local DNS caches on 53, or mDNS loops).

### 6.3 Security Clearance Profile

* Running `portmaster` as a standard user profile grants full authority to discover and manipulate any network sockets and process blocks owned by that same user identity.

* Querying or terminating system processes or background daemons owned by distinct system identities `root`, `SYSTEM`, `NetworkService`) will fail gracefully unless explicitly executed via elevated permission blocks `sudo` or administrative command loops).

```
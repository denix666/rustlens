# Rustlens
Project in a progress!

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen?style=plastic)](https://github.com/denix666/rustlens)
[![Latest Release](https://img.shields.io/github/v/release/denix666/rustlens?style=plastic)](https://github.com/denix666/rustlens/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=plastic)](https://opensource.org/licenses/MIT)

A powerful and intuitive ui tool for managing your Kubernetes (k8s) cluster. This program allows you to seamlessly interact with your cluster resources, streamlining your development and deployment workflows.

---

## Features

* **View Resources:** List nodes, pods, services, and deployments across all namespaces.
* **Inspect Resources:** Get detailed descriptions of specific cluster resources (`describe`).
* **Manage Pods:** View real-time logs from any running pod.
* **Control Deployments:** Scale your deployments up or down.
* **Inspect logs:** Log parser with your own plugins for search specific patterns.
* **Converters and other DevOps tools:** Base64 decoder, JWT decoder, IP calculator, YAML to JSON converter and more...
* And much more!

---

## Prerequisites

For a successful connection to your Kubernetes cluster, you must have a valid configuration file.

* **Configuration File:** The program requires a file named `.kube/config` to be present in your home directory.
    * On **macOS/Linux**, this path is `~/.kube/config`
    * On **Windows**, this path is `%USERPROFILE%\.kube\config` (not tested yet)

Please ensure your cluster connection details are correctly specified in this file.

There are plans in the future to implement multicluster connection.

---

## Log parser plugins
* **Plugin structure:**
```
name: "plugin-name"
description: "Plugin description"
rules:
  - id: "plugin-id"
    title: "Plugin title"
    patterns:
      - "Some regexp"
      - "some other pattern"
      - "and more patterns"
    level: "error"
    message: "Found error in log"
    recommendation: "Fix error and restart pod"
    context_lines: 0
    threshold: 1
```
* name - Should not contain spaces
* description - Optional
* id - Should be unique and not contain spaces
* title - Optional
* patterns - Patterns to search in the logs (it will search from the end to the head)
* level - Can be "error" or "warn"
* message - Message that will be displayed
* recommendation - Recommendation of how to fix the issue
* context_lines - How many matched lines you want to see in parser (0 will not show any)
* threshold - How many patterns should be found in log to trigger message

**Each plugin** should be stored in `~/.local/share/rustlens/plugins` directory as *.yaml file.

---

## Build from source

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/denix666/rustlens.git
    ```

2.  **Navigate to the project directory:**
    ```bash
    cd rustlens
    ```

3.  **Build the application:**
    ```bash
    cargo build --release
    ```
---

## Cross-Platform Compatibility
This application is designed to be cross-platform and should run on both Windows and macOS. However, please note that it has not yet been formally tested on these platforms. We appreciate any feedback on its performance in these environments.

---

## Contributing
Contributions are welcome! Please feel free to open an issue or submit a pull request.

---

## License
[MIT License](https://opensource.org/licenses/MIT)

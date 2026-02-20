==================
Contributing Guide
==================

Thank you for your interest in contributing to Kiln! This guide covers the process for making contributions.

.. toctree::
   :maxdepth: 2

   code_style
   commit_guidelines
   pull_request_process
   testing_requirements
   documentation_guidelines

Getting Started
===============

Before You Begin
----------------

1. Read through our :doc:`../setup/index` guide
2. Understand the :doc:`../../architecture/index` 
3. Browse existing :doc:`../../examples/index`
4. Check open issues on GitHub

Development Process
===================

1. **Fork and Clone**

   .. code-block:: bash

      # Fork on GitHub, then clone your fork
      git clone https://github.com/pulseengine/kiln.git
      cd kiln

2. **Create Feature Branch**

   .. code-block:: bash

      # Create and switch to feature branch
      git checkout -b feature/your-feature-name

3. **Make Changes**

   * Follow our :doc:`code_style` guidelines
   * Add comprehensive tests
   * Update documentation as needed
   * Use :doc:`commit_guidelines` for commit messages

4. **Test Locally**

   .. code-block:: bash

      # Format code and run checks
      cargo-kiln check

      # Run tests
      cargo-kiln test

      # Run full CI checks
      cargo-kiln ci

5. **Submit Pull Request**

   * Follow our :doc:`pull_request_process`
   * Include clear description of changes
   * Reference any related issues

Code Guidelines
===============

Safety and Quality
------------------

* **Safety First**: Document all panic conditions
* **Test Coverage**: Add tests for all new functionality  
* **Performance**: Consider no_std and embedded constraints
* **Documentation**: Update docs for public APIs

Rust Standards
--------------

* Follow Rust API Guidelines
* Use conventional naming (snake_case, CamelCase)
* Prefer explicit error handling over panics
* Use appropriate visibility modifiers

Code of Conduct
===============

This project follows the Contributor Covenant Code of Conduct:

**Our Standards**

* Use welcoming and inclusive language
* Be respectful of differing viewpoints
* Accept constructive criticism gracefully
* Focus on what's best for the community
* Show empathy towards community members

**Unacceptable Behavior**

* Sexualized language or unwelcome advances
* Trolling, insulting comments, or personal attacks
* Public or private harassment
* Publishing private information without permission

**Enforcement**

Report unacceptable behavior to the project maintainers. All complaints will be reviewed and investigated promptly and fairly.

Getting Help
============

* **Questions**: Use GitHub Discussions
* **Bugs**: File an issue with reproduction steps
* **Features**: Discuss in issues before implementing
* **Security**: Follow responsible disclosure process

Next Steps
==========

* Review :doc:`code_style` for specific style requirements
* Check :doc:`testing_requirements` for test expectations
* See :doc:`../build_system/index` for build details
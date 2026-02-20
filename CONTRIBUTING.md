# Contributing

Thank you for your interest in contributing to Kiln!

## Quick Start

For complete contribution guidelines, please see our comprehensive documentation:

**📚 [Developer Documentation](./docs/source/developer/index.rst)**

### Essential Links

- **[Development Setup](./docs/source/developer/setup/index.rst)** - Environment setup and toolchain installation
- **[Contributing Guide](./docs/source/developer/contributing/index.rst)** - Complete contribution process
- **[Build System](./docs/source/developer/build_system/index.rst)** - Build commands and configuration
- **[Testing](./docs/source/developer/testing/index.rst)** - Test requirements and procedures

### Quick Commands

```bash
# Install cargo-kiln
cargo install --path cargo-kiln

# Setup development environment
cargo-kiln setup --check             # Check tool dependencies
cargo-kiln setup --all               # Install tools and setup git hooks
cargo-kiln build
cargo-kiln test

# Tool version management
cargo-kiln tool-versions check       # Verify tool versions
cargo-kiln tool-versions check --verbose  # Detailed version information

# Before submitting PR
cargo-kiln check
cargo-kiln ci

# Additional development commands
cargo-kiln no-std                   # Verify no_std compatibility
cargo-kiln check --strict            # Check code formatting and linting
cargo-kiln coverage --html           # Generate test coverage
cargo-kiln docs --private            # Build and validate documentation
cargo-kiln verify-matrix --report    # Run comprehensive verification
```

## Code of Conduct

<!-- TODO: Link to or include a Code of Conduct if applicable. -->
This project and everyone participating in it is governed by the following Code of Conduct. By participating, you are expected to uphold this code. Please report unacceptable behavior.

**Our Pledge**

In the interest of fostering an open and welcoming environment, we as contributors and maintainers pledge to make participation in our project and our community a harassment-free experience for everyone, regardless of age, body size, disability, ethnicity, sex characteristics, gender identity and expression, level of experience, education, socio-economic status, nationality, personal appearance, race, religion, or sexual identity and orientation.

**Our Standards**

Examples of behavior that contributes to creating a positive environment include:

*   Using welcoming and inclusive language
*   Being respectful of differing viewpoints and experiences
*   Gracefully accepting constructive criticism
*   Focusing on what is best for the community
*   Showing empathy towards other community members

Examples of unacceptable behavior by participants include:

*   The use of sexualized language or imagery and unwelcome sexual attention or advances
*   Trolling, insulting/derogatory comments, and personal or political attacks
*   Public or private harassment
*   Publishing others' private information, such as a physical or electronic address, without explicit permission
*   Other conduct which could reasonably be considered inappropriate in a professional setting

**Our Responsibilities**

Project maintainers are responsible for clarifying the standards of acceptable behavior and are expected to take appropriate and fair corrective action in response to any instances of unacceptable behavior.

Project maintainers have the right and responsibility to remove, edit, or reject comments, commits, code, wiki edits, issues, and other contributions that are not aligned to this Code of Conduct, or to ban temporarily or permanently any contributor for other behaviors that they deem inappropriate, threatening, offensive, or harmful.

**Scope**

This Code of Conduct applies both within project spaces and in public spaces when an individual is representing the project or its community. Examples of representing a project or community include using an official project e-mail address, posting via an official social media account, or acting as an appointed representative at an online or offline event. Representation of the project may be further defined and clarified by project maintainers.

**Enforcement**

Instances of abusive, harassing, or otherwise unacceptable behavior may be reported by contacting the project team at [INSERT CONTACT METHOD HERE]. All complaints will be reviewed and investigated and will result in a response that is deemed necessary and appropriate to the circumstances. The project team is obligated to maintain confidentiality with regard to the reporter of an incident. Further details of specific enforcement policies may be posted separately.

Project maintainers who do not follow or enforce the Code of Conduct in good faith may face temporary or permanent repercussions as determined by other members of the project's leadership.

**Attribution**

This Code of Conduct is adapted from the [Contributor Covenant](https://www.contributor-covenant.org), version 2.0, available at [https://www.contributor-covenant.org/version/2/0/code_of_conduct.html](https://www.contributor-covenant.org/version/2/0/code_of_conduct.html). 
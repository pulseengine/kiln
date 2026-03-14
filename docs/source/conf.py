import os
import sys
import platform
import json
import subprocess
import pathlib
import re
sys.path.insert(0, os.path.abspath('../..'))

project = 'Kiln (WebAssembly Runtime)'
copyright = '2025, Kiln Contributors'
author = 'Kiln Contributors'
# release = '0.1.0' # This will be set dynamically

# Version configuration
# DOCS_VERSION is set by the Dagger pipeline (e.g., "main", "v0.1.0", "local")
# It's already used for current_version for the switcher.
# We'll use it to set 'release' and 'version' for Sphinx metadata.

# Default to 'main' if DOCS_VERSION is not set (e.g. local manual build)
# The Dagger pipeline will always set DOCS_VERSION.
docs_build_env_version = os.environ.get('DOCS_VERSION', 'main')

if docs_build_env_version.lower() in ['main', 'local']:
    release = 'dev'  # Full version string for 'main' or 'local'
    version = 'dev'  # Shorter X.Y version
else:
    # Process semantic versions like "v0.1.0" or "0.1.0"
    parsed_release = docs_build_env_version.lstrip('v')
    release = parsed_release  # Full version string, e.g., "0.1.0"
    version_parts = parsed_release.split('.')
    if len(version_parts) >= 2:
        version = f"{version_parts[0]}.{version_parts[1]}"  # Shorter X.Y, e.g., "0.1"
    else:
        version = parsed_release  # Fallback if not in X.Y.Z or similar format

# current_version is used by the theme for matching in the version switcher
current_version = os.environ.get('DOCS_VERSION', 'main')
# version_path_prefix is used by the theme to construct the URL to switcher.json
# The Dagger pipeline sets this to "/"
version_path_prefix = os.environ.get('DOCS_VERSION_PATH_PREFIX', '/')

# Function to get available versions
def get_versions():
    versions = ['main']
    try:
        # Get all tags
        result = subprocess.run(['git', 'tag'], stdout=subprocess.PIPE, universal_newlines=True)
        if result.returncode == 0:
            # Only include semantic version tags (x.y.z)
            tags = result.stdout.strip().split('\n')
            for tag in tags:
                if re.match(r'^\d+\.\d+\.\d+$', tag):
                    versions.append(tag)
    except Exception as e:
        print(f"Error getting versions: {e}")
    
    return sorted(versions, key=lambda v: v if v == 'main' else [int(x) for x in v.split('.')])

# Available versions for the switcher
versions = get_versions()

# Write versions data for the index page to use for redirection
versions_data = {
    'current_version': current_version,
    'versions': versions,
    'version_path_prefix': version_path_prefix
}

# Ensure _static directory exists
os.makedirs(os.path.join(os.path.dirname(__file__), '_static'), exist_ok=True)

# Write versions data to a JSON file
with open(os.path.join(os.path.dirname(__file__), '_static', 'versions.json'), 'w') as f:
    json.dump(versions_data, f)

# Add version data to the context for templates
html_context = {
    'current_version': current_version,
    'versions': versions,
    'version_path_prefix': version_path_prefix
}

# Custom monkeypatch to handle NoneType in names
def setup(app):
    from sphinx.domains.std import StandardDomain
    old_process_doc = StandardDomain.process_doc
    
    def patched_process_doc(self, env, docname, document):
        try:
            return old_process_doc(self, env, docname, document)
        except TypeError as e:
            if "'NoneType' object is not subscriptable" in str(e):
                print(f"WARNING: Caught TypeError in {docname}. This indicates a node with missing 'names' attribute.")
                return
            raise
    
    StandardDomain.process_doc = patched_process_doc
    
    # Add our custom CSS
    app.add_css_file('css/custom.css')
    
    # Add our custom JavaScript for code copy
    app.add_js_file('js/code-copy.js')
    
    return {'version': '0.1', 'parallel_read_safe': True}

extensions = [
    'sphinx.ext.autodoc',
    'sphinx.ext.viewcode',
    'sphinx.ext.napoleon',
    'myst_parser',
    'sphinxcontrib.plantuml',
    "sphinxcontrib_rust",
    'sphinx_design',
]

templates_path = ['_templates']
exclude_patterns = []

# Change theme from sphinx_book_theme to pydata_sphinx_theme
html_theme = 'pydata_sphinx_theme'
html_static_path = ['_static']

# Configure theme options
html_theme_options = {
    # Configure the version switcher
    "switcher": {
        "json_url": f"{version_path_prefix}switcher.json",
        "version_match": current_version,
    },
    # Put logo on far left, search and utilities on the right  
    "navbar_start": ["navbar-logo"],
    # Keep center empty to move main nav to sidebar
    "navbar_center": [],
    # Group version switcher with search and theme switcher on the right
    "navbar_end": ["version-switcher", "search-button", "theme-switcher"], 
    # Test configuration - disable in production
    "check_switcher": True,
    # Control navigation bar behavior
    "navbar_align": "left", # Align content to left
    "use_navbar_nav_drop_shadow": True,
    # Control the sidebar navigation
    "navigation_with_keys": True,
    "show_nav_level": 2, # Show more levels in the left sidebar nav
    "show_toc_level": 2, # On-page TOC levels
    # Collapse navigation to only show current page's children in sidebar
    "collapse_navigation": True, # Set to False if you want full tree always visible
    "show_prev_next": True,
}

# Sidebar configuration
html_sidebars = {
    "**": ["sidebar-nav-bs.html", "sidebar-ethical-ads.html"] # Ensures main nav is in sidebar
}

# ADDED FOR DEBUGGING
print(f"[DEBUG] conf.py: current_version (for version_match) = '{current_version}'")
print(f"[DEBUG] conf.py: version_path_prefix = '{version_path_prefix}'")
print(f"[DEBUG] conf.py: Calculated switcher json_url = '{html_theme_options['switcher']['json_url']}'")
# END DEBUGGING

# PlantUML configuration
# Using the installed plantuml executable
plantuml = 'plantuml'
plantuml_output_format = 'svg'
plantuml_latex_output_format = 'pdf'

# Make PlantUML work cross-platform
if platform.system() == "Windows":
    # Windows may need the full path to the plantuml.jar or plantuml.bat
    plantuml = os.environ.get('PLANTUML_PATH', 'plantuml')
elif platform.system() == "Darwin":  # macOS
    # macOS typically uses Homebrew installation
    plantuml = os.environ.get('PLANTUML_PATH', 'plantuml')
    # Add debug info
    print(f"PlantUML path on macOS: {plantuml}")
    print(f"PlantUML exists: {os.path.exists(plantuml) if os.path.isabs(plantuml) else 'checking PATH'}")
elif platform.system() == "Linux":
    # Linux installation path
    plantuml = os.environ.get('PLANTUML_PATH', 'plantuml')

# Allow customization through environment variables
plantuml_output_format = os.environ.get('PLANTUML_FORMAT', 'svg')


# Requirement/safety traceability has been migrated to rivet (safety/ directory).
# See: rivet validate, rivet coverage, rivet serve

# Initialize source_suffix before attempting to modify it
source_suffix = {
    '.rst': 'restructuredtext',
    '.md': 'markdown',
    # Add .txt if you use it for markdown, or remove if not needed
    # '.txt': 'markdown', 
}

# Ensure myst_parser is configured for .md files (it should be by default if in extensions)
# but explicitly adding/checking source_suffix is good practice.
if isinstance(source_suffix, dict):
    if '.md' not in source_suffix:
        source_suffix['.md'] = 'markdown'
elif isinstance(source_suffix, list): # if it's a list of extensions
    if '.md' not in source_suffix:
        source_suffix.append('.md')
else: # if it's a single string or not set as expected
    source_suffix = {
        '.rst': 'restructuredtext',
        '.md': 'markdown',
    }


# Rust documentation configuration
# Start with core working crates first
rust_crates = {
    "kiln-error": "/kiln/kiln-error",
    "kiln-foundation": "/kiln/kiln-foundation",
    "kiln-sync": "/kiln/kiln-sync",
    "kiln-logging": "/kiln/kiln-logging",
    "kiln-math": "/kiln/kiln-math",
    "kiln-format": "/kiln/kiln-format",
    "kiln-decoder": "/kiln/kiln-decoder",
    "kiln-host": "/kiln/kiln-host",
    "kiln-intercept": "/kiln/kiln-intercept",
    # Test one by one:
    # "kiln-instructions": "/kiln/kiln-instructions",
    # "kiln-platform": "/kiln/kiln-platform",
    # Temporarily disable complex crates that might have build issues:
    # "kiln-foundation": "/kiln/kiln-foundation", 
    # "kiln-format": "/kiln/kiln-format",
    # "kiln-decoder": "/kiln/kiln-decoder",
    # "kiln-host": "/kiln/kiln-host",
    # "kiln-intercept": "/kiln/kiln-intercept",
    # "kiln-instructions": "/kiln/kiln-instructions",
    # "kiln-platform": "/kiln/kiln-platform",
    # "kiln-runtime": "/kiln/kiln-runtime",
    # "kiln-component": "/kiln/kiln-component",
    # "kiln": "/kiln/kiln",
    # "kilnd": "/kiln/kilnd",
    # "kiln-debug": "/kiln/kiln-debug",
    # "kiln-verification-tool": "/kiln/kiln-verification-tool",
    # "kiln-test-registry": "/kiln/kiln-test-registry",
}

# Directory where sphinx-rustdocgen will place generated .md files.
# This path is relative to conf.py (docs/source/)
rust_doc_dir = "_generated_rust_docs" 

# Assuming Rust doc comments are written in Markdown.
# If they are in reStructuredText, this can be set to "rst" or omitted (default).
rust_rustdoc_fmt = "md"
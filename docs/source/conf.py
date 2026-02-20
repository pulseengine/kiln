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
    
    # Register the dynamic function for extracting requirements
    from sphinx_needs.api.configuration import add_dynamic_function
    add_dynamic_function(app, extract_reqs)
    
    return {'version': '0.1', 'parallel_read_safe': True}

extensions = [
    'sphinx.ext.autodoc',
    'sphinx.ext.viewcode',
    'sphinx.ext.napoleon',
    'sphinx_needs',
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

# Sphinx-needs configuration
needs_types = [
    dict(directive="req", title="Requirement", prefix="REQ_", color="#BFD8D2", style="node"),
    dict(directive="spec", title="Specification", prefix="SPEC_", color="#FEDCD2", style="node"),
    dict(directive="impl", title="Implementation", prefix="IMPL_", color="#DF744A", style="node"),
    dict(directive="test", title="Test Case", prefix="T_", color="#DCB239", style="node"),
    dict(directive="safety", title="Safety", prefix="SAFETY_", color="#FF5D73", style="node"),
    dict(directive="qual", title="Qualification", prefix="QUAL_", color="#9370DB", style="node"),
    dict(directive="constraint", title="Constraint", prefix="CNST_", color="#4682B4", style="node"),
    dict(directive="panic", title="Panic", prefix="KILNQ_", color="#E74C3C", style="node"),
    dict(directive="src",  title="Source file",  prefix="SRC_", color="#C6C6FF", style="node"),
    # Architecture-specific types
    dict(directive="arch_component", title="Architectural Component", prefix="ARCH_COMP_", color="#FF6B6B", style="node"),
    dict(directive="arch_interface", title="Interface", prefix="ARCH_IF_", color="#4ECDC4", style="node"),
    dict(directive="arch_decision", title="Design Decision", prefix="ARCH_DEC_", color="#45B7D1", style="node"),
    dict(directive="arch_constraint", title="Design Constraint", prefix="ARCH_CON_", color="#96CEB4", style="node"),
    dict(directive="arch_pattern", title="Design Pattern", prefix="ARCH_PAT_", color="#FECA57", style="node"),
]

# Add ID regex pattern for sphinx-needs
needs_id_regex = '^[A-Z0-9_]{5,}$'

# Add option specs to register additional options for directives
needs_extra_options = [
    'rationale', 
    'verification', 
    'mitigation', 
    'implementation', 
    'safety_impact',
    'item_status',
    'handling_strategy',
    'last_updated',
    'file',
    'implements',
    # Architecture-specific options
    'crate',
    'provides',
    'requires',
    'allocated_requirements',
    'environment',
    'variant_of',
    'impacts',
    'deciders',
    'alternatives',
    'stability',
    'protocol',
]

# Allow all sphinx-needs options for all directives
needs_allow_unsafe_options = True

# Disable warnings for unknown link targets to avoid the many outgoing link warnings
needs_warnings_always_warn = False

# Custom sphinx-needs templates for qualification and safety
needs_templates = {
    'safety_template': '**Hazard**: {{content}}\n\n**Mitigation**: {{mitigation}}',
    'qualification_template': '**Status**: {{item_status}}\n\n**Implementation**: {{implementation}}',
    'constraint_template': '**Constraint**: {{content}}\n\n**Rationale**: {{rationale}}\n\n**Verification**: {{verification}}',
    'panic_template': '**Panic Condition**: {{content}}\n\n**Safety Impact**: {{safety_impact}}\n\n**Status**: {{item_status}}\n\n**Handling Strategy**: {{handling_strategy}}',
}

# Tags for filtering and displaying panic entries
needs_tags = [
    dict(name="panic", description="Panic documentation entry", bgcolor="#E74C3C"),
    dict(name="low", description="Low safety impact", bgcolor="#2ECC71"),
    dict(name="medium", description="Medium safety impact", bgcolor="#F39C12"),
    dict(name="high", description="High safety impact", bgcolor="#E74C3C"),
    dict(name="unknown", description="Unknown safety impact", bgcolor="#95A5A6"),
    # Architecture tags
    dict(name="core", description="Core architecture component", bgcolor="#FF6B6B"),
    dict(name="portability", description="Multi-platform portability", bgcolor="#4ECDC4"),
    dict(name="safety", description="Safety-critical component", bgcolor="#FF5D73"),
    dict(name="performance", description="Performance-critical component", bgcolor="#FECA57"),
    dict(name="testing", description="Testing and verification", bgcolor="#96CEB4"),
]

# Configure needs roles for referencing 
needs_role_need_template = "{title} ({id})"
needs_role_need_max_title_length = 30

needs_id_length = 7
needs_title_optional = True
needs_file_pattern = '**/*.rst'

# New extra links configuration
needs_extra_links = [
    dict(
        option   = "realizes",
        incoming = "is realized by",
        outgoing = "realizes",
        style    = "solid,#006A6A",
    ),
]

# Regular expression for finding requirement IDs
REQ_RE = re.compile(r"SW-REQ-ID\\s*:\\s*(REQ_\\w+)", re.I)

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

# Dynamic function to extract requirement IDs from a file
def extract_reqs(app, need, needs, *args, **kwargs):
    """
    Return all REQ_xxx IDs that occur in the file given via :file:.
    Called as a *dynamic function* during the build.
    """
    relative_file_path_from_doc_source = need.get("file")
    if not relative_file_path_from_doc_source:
        return ""

    # Construct the absolute path to the source file.
    # app.confdir is the directory of conf.py (e.g., /path/to/workspace/docs/source)
    # relative_file_path_from_doc_source is like '../../kiln/src/some_file.rs'
    # So, Path(app.confdir) / relative_file_path_from_doc_source gives the absolute path.
    absolute_src_file_path = (pathlib.Path(app.confdir) / relative_file_path_from_doc_source).resolve()
    
    try:
        text = absolute_src_file_path.read_text(errors="ignore")
        ids  = REQ_RE.findall(text)
        return ";".join(sorted(set(ids)))  # needs wants ';' as separator
    except FileNotFoundError:
        print(f"WARNING: [extract_reqs] File not found: {absolute_src_file_path} (original path in need: {relative_file_path_from_doc_source})")
        return ""
    except Exception as e:
        print(f"ERROR: [extract_reqs] Could not read file {absolute_src_file_path}: {e}")
        return ""

# Configuration to make specific strings in RST linkable
needs_string_links = {
    # Link REQ_XXX to its definition
    "req_inline": {
        "regex": r"(?P<value>REQ_\w+)",
        "link_url": "#{{value}}",
        "link_name": "{{value}}",
        "options": [],
    },
    # Link file paths in :file: option to GitHub
    "source_file_link": {
        "regex": r"^(?P<value>(?:\.\.\/)*[a-zA-Z0-9_\-\/]+\.rs)$",
        "link_url": "https://github.com/pulseengine/kiln/blob/main/{{value.replace('../../', '')}}",
        "link_name": "{{value}}",
        "options": ["file"],
    }
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
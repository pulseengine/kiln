// Interactive features for cargo-wrt HTML reports

document.addEventListener('DOMContentLoaded', function() {
    // Initialize all interactive features
    initializeTableSorting();
    initializeFiltering();
    initializeTooltips();
    initializeCollapsibleSections();
    initializeSearchFunctionality();
});

// Table sorting functionality
function initializeTableSorting() {
    const tables = document.querySelectorAll('.requirements-table');
    
    tables.forEach(table => {
        const headers = table.querySelectorAll('thead th');
        
        headers.forEach((header, index) => {
            header.style.cursor = 'pointer';
            header.style.userSelect = 'none';
            header.setAttribute('title', 'Click to sort');
            
            header.addEventListener('click', () => {
                sortTable(table, index);
            });
        });
    });
}

function sortTable(table, columnIndex) {
    const tbody = table.querySelector('tbody');
    const rows = Array.from(tbody.querySelectorAll('tr'));
    const isNumeric = isColumnNumeric(rows, columnIndex);
    const isAscending = !table.dataset.sortAsc || table.dataset.sortAsc === 'false';
    
    rows.sort((a, b) => {
        const aText = a.cells[columnIndex].textContent.trim();
        const bText = b.cells[columnIndex].textContent.trim();
        
        let comparison;
        if (isNumeric) {
            comparison = parseFloat(aText) - parseFloat(bText);
        } else {
            comparison = aText.localeCompare(bText);
        }
        
        return isAscending ? comparison : -comparison;
    });
    
    // Update sort direction
    table.dataset.sortAsc = isAscending.toString();
    
    // Clear existing sort indicators
    table.querySelectorAll('thead th').forEach(th => {
        th.classList.remove('sort-asc', 'sort-desc');
    });
    
    // Add sort indicator to current column
    const currentHeader = table.querySelectorAll('thead th')[columnIndex];
    currentHeader.classList.add(isAscending ? 'sort-asc' : 'sort-desc');
    
    // Reorder rows
    rows.forEach(row => tbody.appendChild(row));
}

function isColumnNumeric(rows, columnIndex) {
    for (let i = 0; i < Math.min(5, rows.length); i++) {
        const cellText = rows[i].cells[columnIndex].textContent.trim();
        const numericValue = parseFloat(cellText);
        if (!isNaN(numericValue) && isFinite(numericValue)) {
            return true;
        }
    }
    return false;
}

// Filtering functionality
function initializeFiltering() {
    const filterContainer = document.createElement('div');
    filterContainer.className = 'filter-controls';
    filterContainer.innerHTML = `
        <div class="filter-group">
            <label for="asil-filter">Filter by ASIL Level:</label>
            <select id="asil-filter">
                <option value="">All ASIL Levels</option>
                <option value="qm">QM</option>
                <option value="a">ASIL-A</option>
                <option value="b">ASIL-B</option>
                <option value="c">ASIL-C</option>
                <option value="d">ASIL-D</option>
            </select>
        </div>
        <div class="filter-group">
            <label for="status-filter">Filter by Status:</label>
            <select id="status-filter">
                <option value="">All Statuses</option>
                <option value="verified">Verified</option>
                <option value="implemented">Implemented</option>
                <option value="partial">Partial</option>
                <option value="pending">Pending</option>
            </select>
        </div>
    `;
    
    const table = document.querySelector('.requirements-table');
    if (table) {
        table.parentNode.insertBefore(filterContainer, table);
        
        document.getElementById('asil-filter').addEventListener('change', applyFilters);
        document.getElementById('status-filter').addEventListener('change', applyFilters);
    }
}

function applyFilters() {
    const asilFilter = document.getElementById('asil-filter').value.toLowerCase();
    const statusFilter = document.getElementById('status-filter').value.toLowerCase();
    const rows = document.querySelectorAll('.requirements-table tbody tr');
    
    rows.forEach(row => {
        const asilCell = row.querySelector('.asil-level');
        const statusCell = row.querySelector('.req-status span');
        
        const asilText = asilCell ? asilCell.textContent.toLowerCase() : '';
        const statusText = statusCell ? statusCell.textContent.toLowerCase() : '';
        
        const asilMatch = !asilFilter || asilText.includes(asilFilter);
        const statusMatch = !statusFilter || statusText.includes(statusFilter);
        
        row.style.display = (asilMatch && statusMatch) ? '' : 'none';
    });
}

// Tooltip functionality
function initializeTooltips() {
    const tooltipElements = document.querySelectorAll('[title]');
    
    tooltipElements.forEach(element => {
        element.addEventListener('mouseenter', showTooltip);
        element.addEventListener('mouseleave', hideTooltip);
    });
}

function showTooltip(event) {
    const element = event.target;
    const title = element.getAttribute('title');
    
    if (!title) return;
    
    const tooltip = document.createElement('div');
    tooltip.className = 'custom-tooltip';
    tooltip.textContent = title;
    
    document.body.appendChild(tooltip);
    
    const rect = element.getBoundingClientRect();
    tooltip.style.left = rect.left + (rect.width / 2) - (tooltip.offsetWidth / 2) + 'px';
    tooltip.style.top = rect.top - tooltip.offsetHeight - 5 + 'px';
    
    element.removeAttribute('title');
    element.dataset.originalTitle = title;
}

function hideTooltip(event) {
    const element = event.target;
    const tooltip = document.querySelector('.custom-tooltip');
    
    if (tooltip) {
        tooltip.remove();
    }
    
    if (element.dataset.originalTitle) {
        element.setAttribute('title', element.dataset.originalTitle);
        delete element.dataset.originalTitle;
    }
}

// Collapsible sections
function initializeCollapsibleSections() {
    const sections = document.querySelectorAll('[data-collapsible]');
    
    sections.forEach(section => {
        const header = section.querySelector('h2, h3');
        if (header) {
            header.style.cursor = 'pointer';
            header.style.userSelect = 'none';
            header.innerHTML += ' <span class="collapse-indicator">▼</span>';
            
            header.addEventListener('click', () => {
                toggleSection(section);
            });
        }
    });
}

function toggleSection(section) {
    const content = section.querySelector('[data-collapsible-content]');
    const indicator = section.querySelector('.collapse-indicator');
    
    if (content.style.display === 'none') {
        content.style.display = '';
        indicator.textContent = '▼';
    } else {
        content.style.display = 'none';
        indicator.textContent = '▶';
    }
}

// Search functionality
function initializeSearchFunctionality() {
    const searchContainer = document.createElement('div');
    searchContainer.className = 'search-container';
    searchContainer.innerHTML = `
        <input type="text" id="search-input" placeholder="Search requirements..." />
        <button id="clear-search">Clear</button>
    `;
    
    const table = document.querySelector('.requirements-table');
    if (table) {
        table.parentNode.insertBefore(searchContainer, table);
        
        document.getElementById('search-input').addEventListener('input', performSearch);
        document.getElementById('clear-search').addEventListener('click', clearSearch);
    }
}

function performSearch() {
    const searchTerm = document.getElementById('search-input').value.toLowerCase();
    const rows = document.querySelectorAll('.requirements-table tbody tr');
    
    rows.forEach(row => {
        const text = row.textContent.toLowerCase();
        row.style.display = text.includes(searchTerm) ? '' : 'none';
    });
}

function clearSearch() {
    document.getElementById('search-input').value = '';
    const rows = document.querySelectorAll('.requirements-table tbody tr');
    rows.forEach(row => {
        row.style.display = '';
    });
}

// Add CSS for interactive features
const style = document.createElement('style');
style.textContent = `
    .filter-controls {
        display: flex;
        gap: 1rem;
        margin-bottom: 1rem;
        padding: 1rem;
        background: var(--surface-color);
        border-radius: var(--border-radius);
        border: 1px solid var(--border-color);
    }
    
    .filter-group {
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
    }
    
    .filter-group label {
        font-size: 0.875rem;
        font-weight: 500;
        color: var(--text-muted);
    }
    
    .filter-group select {
        padding: 0.5rem;
        border: 1px solid var(--border-color);
        border-radius: 0.25rem;
        background: var(--background-color);
        color: var(--text-color);
    }
    
    .search-container {
        display: flex;
        gap: 0.5rem;
        margin-bottom: 1rem;
        padding: 1rem;
        background: var(--surface-color);
        border-radius: var(--border-radius);
        border: 1px solid var(--border-color);
    }
    
    .search-container input {
        flex: 1;
        padding: 0.5rem;
        border: 1px solid var(--border-color);
        border-radius: 0.25rem;
        background: var(--background-color);
        color: var(--text-color);
    }
    
    .search-container button {
        padding: 0.5rem 1rem;
        border: 1px solid var(--border-color);
        border-radius: 0.25rem;
        background: var(--primary-color);
        color: white;
        cursor: pointer;
    }
    
    .custom-tooltip {
        position: absolute;
        background: var(--text-color);
        color: var(--background-color);
        padding: 0.5rem;
        border-radius: 0.25rem;
        font-size: 0.875rem;
        z-index: 1000;
        max-width: 200px;
        text-align: center;
    }
    
    .sort-asc::after {
        content: ' ↑';
    }
    
    .sort-desc::after {
        content: ' ↓';
    }
    
    .collapse-indicator {
        font-size: 0.75em;
        margin-left: 0.5rem;
    }
    
    @media (max-width: 768px) {
        .filter-controls {
            flex-direction: column;
        }
        
        .search-container {
            flex-direction: column;
        }
    }
`;
document.head.appendChild(style);
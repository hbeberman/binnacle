/**
 * Sidebar Component
 * 
 * Collapsible sidebar with:
 * - Header with icon and title
 * - Search input
 * - Collapsible sections (Agents, Nodes, Edges)
 * - Persisted collapse state to localStorage
 */

const SIDEBAR_COLLAPSE_KEY = 'binnacle_sidebar_collapsed';

/**
 * Load sidebar collapse state from localStorage
 * @returns {Object} Map of section keys to collapsed state
 */
function loadSidebarCollapseState() {
    try {
        const saved = localStorage.getItem(SIDEBAR_COLLAPSE_KEY);
        return saved ? JSON.parse(saved) : {};
    } catch {
        return {};
    }
}

/**
 * Save sidebar collapse state to localStorage
 * @param {Object} state - Map of section keys to collapsed state
 */
function saveSidebarCollapseState(state) {
    try {
        localStorage.setItem(SIDEBAR_COLLAPSE_KEY, JSON.stringify(state));
    } catch {
        // Ignore localStorage errors
    }
}

/**
 * Create sidebar HTML structure
 * @returns {HTMLElement} Sidebar container element
 */
export function createSidebar() {
    const sidebar = document.createElement('div');
    sidebar.className = 'agents-sidebar';
    sidebar.id = 'agents-sidebar';
    
    sidebar.innerHTML = `
        <div class="agents-sidebar-header">
            <span class="agents-sidebar-icon">🤖</span>
            <span class="agents-sidebar-title">Sidebar</span>
        </div>
        <div class="agents-sidebar-content" id="agents-sidebar-content">
            <!-- Sidebar search input -->
            <div class="sidebar-search-container">
                <input class="sidebar-search" id="sidebar-search" type="text" placeholder="Filter…" autocomplete="off" spellcheck="false" />
            </div>
            <!-- Agents section -->
            <div class="sidebar-section collapsible" id="agents-section" data-section="agents">
                <div class="sidebar-section-title">Agents <span class="sidebar-section-toggle">▼</span></div>
                <div class="sidebar-section-content">
                    <div class="agents-sidebar-list" id="agents-sidebar-list"></div>
                </div>
            </div>
            <!-- Node filters section -->
            <div class="sidebar-section collapsible" data-section="nodes">
                <div class="sidebar-section-title">Nodes <span class="sidebar-section-toggle">▼</span></div>
                <div class="sidebar-section-content">
                    <div class="sidebar-filter-group" id="sidebar-node-filters">
                        <!-- Node type filter buttons will be populated dynamically -->
                    </div>
                </div>
            </div>
            <!-- Edge filters section -->
            <div class="sidebar-section collapsible" data-section="edges">
                <div class="sidebar-section-title">Edges <span class="sidebar-section-toggle">▼</span></div>
                <div class="sidebar-section-content">
                    <div class="sidebar-filter-group" id="sidebar-edge-filters">
                        <!-- Edge type filter buttons will be populated dynamically -->
                    </div>
                </div>
            </div>
        </div>
    `;
    
    return sidebar;
}

/**
 * Initialize collapsible section behavior
 * Restores saved collapse state and adds click handlers
 */
export function initializeCollapsibleSections() {
    const collapsedState = loadSidebarCollapseState();
    const sections = document.querySelectorAll('.sidebar-section.collapsible');
    
    sections.forEach(section => {
        const sectionKey = section.dataset.section;
        const title = section.querySelector('.sidebar-section-title');
        
        // Restore collapsed state
        if (sectionKey && collapsedState[sectionKey]) {
            section.classList.add('collapsed');
        }
        
        // Add click handler to title
        if (title) {
            title.addEventListener('click', () => {
                section.classList.toggle('collapsed');
                
                // Save state
                if (sectionKey) {
                    const newState = loadSidebarCollapseState();
                    newState[sectionKey] = section.classList.contains('collapsed');
                    saveSidebarCollapseState(newState);
                }
            });
        }
    });
}

/**
 * Initialize sidebar search functionality
 * @param {Function} onSearch - Callback function called with search query
 */
export function initializeSidebarSearch(onSearch) {
    const input = document.getElementById('sidebar-search');
    if (!input) return;

    input.addEventListener('input', () => {
        const query = input.value.toLowerCase();
        if (onSearch) {
            onSearch(query);
        }
    });

    input.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') {
            input.value = '';
            if (onSearch) {
                onSearch('');
            }
            e.preventDefault();
        }
    });
}

/**
 * Initialize the sidebar component
 * @param {HTMLElement} container - Container element to append sidebar to
 * @param {Function} onSearch - Optional callback for search functionality
 * @param {Object} options - Optional configuration
 * @param {boolean} options.initializeFilters - Whether to initialize filter buttons (default: false)
 * @returns {HTMLElement} The created sidebar element
 */
export function initializeSidebar(container, onSearch = null, options = {}) {
    const sidebar = createSidebar();
    container.appendChild(sidebar);
    
    // Initialize collapsible sections
    initializeCollapsibleSections();
    
    // Initialize search
    if (onSearch) {
        initializeSidebarSearch(onSearch);
    }
    
    // Initialize filters if requested
    if (options.initializeFilters) {
        // Import and initialize filters dynamically
        import('./filters.js').then(({ initializeNodeTypeFilters, initializeEdgeTypeFilters }) => {
            initializeNodeTypeFilters();
            initializeEdgeTypeFilters();
        }).catch(err => {
            console.error('Failed to load filter components:', err);
        });
    }
    
    return sidebar;
}

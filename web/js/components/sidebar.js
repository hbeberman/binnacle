/**
 * Sidebar Component
 * 
 * Collapsible sidebar with:
 * - Header with icon and title
 * - Search input
 * - Collapsible sections (Agents, Nodes, Edges)
 * - Persisted collapse state to localStorage
 */

import * as State from '../state.js';

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
                <span class="sidebar-search-match-count" id="sidebar-search-match-count"></span>
            </div>
            <!-- Agents section -->
            <div class="sidebar-section collapsible" id="agents-section" data-section="agents">
                <div class="sidebar-section-title">Agents <span class="sidebar-section-toggle">▼</span></div>
                <div class="sidebar-section-content">
                    <div class="agents-sidebar-list" id="agents-sidebar-list"></div>
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
 * Updates state.ui.searchQuery to filter visible items in graph
 * @param {Function} onSearch - Optional callback function called with search query
 */
export function initializeSidebarSearch(onSearch) {
    const input = document.getElementById('sidebar-search');
    if (!input) return;

    const matchCountEl = document.getElementById('sidebar-search-match-count');
    
    // Update match count display
    function updateMatchCountDisplay() {
        if (!matchCountEl) return;
        const searchQuery = State.get('ui.searchQuery') || '';
        const searchMatches = State.get('ui.searchMatches') || [];
        const currentMatchIndex = State.get('ui.currentMatchIndex');
        
        if (searchQuery && searchMatches.length > 0) {
            // Show current position if navigating (currentMatchIndex >= 0), otherwise just count
            if (currentMatchIndex >= 0) {
                matchCountEl.textContent = `${currentMatchIndex + 1}/${searchMatches.length} matching`;
            } else {
                matchCountEl.textContent = `${searchMatches.length} matching`;
            }
            matchCountEl.classList.add('visible');
        } else if (searchQuery && searchMatches.length === 0) {
            matchCountEl.textContent = '0 matching';
            matchCountEl.classList.add('visible');
        } else {
            matchCountEl.textContent = '';
            matchCountEl.classList.remove('visible');
        }
    }
    
    // Subscribe to search state changes
    State.subscribe('ui.searchMatches', updateMatchCountDisplay);
    State.subscribe('ui.currentMatchIndex', updateMatchCountDisplay);
    State.subscribe('ui.searchQuery', updateMatchCountDisplay);
    
    input.addEventListener('input', () => {
        const query = input.value.trim();
        
        // Update state to trigger graph filtering
        State.set('ui.searchQuery', query);
        
        // Call optional callback
        if (onSearch) {
            onSearch(query);
        }
    });

    input.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') {
            input.value = '';
            State.set('ui.searchQuery', '');
            State.set('ui.searchMatches', []);
            State.set('ui.currentMatchIndex', -1);
            if (onSearch) {
                onSearch('');
            }
            e.preventDefault();
        } else if (e.key === 'Enter') {
            // Accept the current filter and exit search mode
            // Filter is already applied via the 'input' event, just blur to indicate acceptance
            input.blur();
            e.preventDefault();
        }
    });
}

/**
 * Initialize the sidebar component
 * @param {HTMLElement} container - Container element to append sidebar to
 * @param {Function} onSearch - Optional callback for search functionality
 * @returns {HTMLElement} The created sidebar element
 */
export function initializeSidebar(container, onSearch = null) {
    const sidebar = createSidebar();
    container.appendChild(sidebar);
    
    // Initialize collapsible sections
    initializeCollapsibleSections();
    
    // Initialize search
    if (onSearch) {
        initializeSidebarSearch(onSearch);
    }
    
    return sidebar;
}

/**
 * Graph Controls Component
 * 
 * Overlay controls for the graph view:
 * - Search input for filtering nodes
 * - Hide completed toggle
 * - Auto-follow configuration
 */

import * as State from '../state.js';

/**
 * Create graph controls HTML structure
 * @returns {HTMLElement} Graph controls container
 */
export function createGraphControls() {
    const controls = document.createElement('div');
    controls.className = 'graph-controls';
    controls.id = 'graph-controls';
    
    controls.innerHTML = `
        <input class="graph-search" id="graph-search" type="text" placeholder="Search nodes…" autocomplete="off" spellcheck="false" />
        <div class="hide-completed-toggle">
            <span class="hide-completed-label">Hide completed</span>
            <button class="hide-completed-switch" id="hide-completed-switch" title="Hide completed nodes (except in active chains)"></button>
        </div>
        <div class="auto-follow-toggle" style="position: relative;">
            <span class="auto-follow-label">Follow</span>
            <select class="follow-type-selector" id="follow-type-selector" title="Select type to follow">
                <option value="">Any</option>
                <option value="task">📋 Tasks</option>
                <option value="bug">🐛 Bugs</option>
                <option value="idea">💡 Ideas</option>
                <option value="agent">🤖 Agents</option>
            </select>
            <button class="auto-follow-switch" id="auto-follow-switch" title="Auto-focus on selected types"></button>
            <button class="auto-follow-config-btn" id="auto-follow-config-btn" title="Configure auto-follow settings">⚙</button>
            <div class="auto-follow-config-popover" id="auto-follow-config-popover">
                <div class="config-popover-title">Auto-Follow Settings</div>
                <div class="config-section">
                    <span class="config-section-label">Auto-follow for new:</span>
                    <div class="config-node-types">
                        <div class="config-node-type-row">
                            <span class="config-node-type-label"><span class="config-node-type-icon">📋</span> Tasks</span>
                            <button class="config-toggle active" id="config-follow-tasks" data-type="task"></button>
                        </div>
                        <div class="config-node-type-row">
                            <span class="config-node-type-label"><span class="config-node-type-icon">🐛</span> Bugs</span>
                            <button class="config-toggle active" id="config-follow-bugs" data-type="bug"></button>
                        </div>
                        <div class="config-node-type-row">
                            <span class="config-node-type-label"><span class="config-node-type-icon">💡</span> Ideas</span>
                            <button class="config-toggle" id="config-follow-ideas" data-type="idea"></button>
                        </div>
                        <div class="config-node-type-row">
                            <span class="config-node-type-label"><span class="config-node-type-icon">🧪</span> Tests</span>
                            <button class="config-toggle" id="config-follow-tests" data-type="test"></button>
                        </div>
                        <div class="config-node-type-row">
                            <span class="config-node-type-label"><span class="config-node-type-icon">📄</span> Docs</span>
                            <button class="config-toggle" id="config-follow-docs" data-type="doc"></button>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    `;
    
    return controls;
}

/**
 * Initialize graph controls functionality
 * @param {HTMLElement} controls - The graph controls element
 * @param {Object} options - Configuration options
 * @param {Function} options.onSearch - Callback for search input changes
 * @param {Function} options.onHideCompletedToggle - Callback for hide completed toggle
 * @param {Function} options.onAutoFollowToggle - Callback for auto-follow toggle
 */
export function initializeGraphControls(controls, options = {}) {
    const {
        onSearch = null,
        onHideCompletedToggle = null,
        onAutoFollowToggle = null
    } = options;
    
    // Initialize search input
    const searchInput = controls.querySelector('#graph-search');
    if (searchInput) {
        searchInput.addEventListener('input', () => {
            const query = searchInput.value.trim();
            State.set('ui.searchQuery', query);
            if (onSearch) {
                onSearch(query);
            }
        });
        
        searchInput.addEventListener('keydown', (e) => {
            if (e.key === 'Escape') {
                searchInput.value = '';
                State.set('ui.searchQuery', '');
                if (onSearch) {
                    onSearch('');
                }
                e.preventDefault();
            }
        });
    }
    
    // Initialize hide completed toggle
    const hideCompletedSwitch = controls.querySelector('#hide-completed-switch');
    if (hideCompletedSwitch) {
        // Load initial state
        const hideCompleted = State.get('ui.hideCompleted') !== false; // Default true
        hideCompletedSwitch.classList.toggle('active', hideCompleted);
        
        hideCompletedSwitch.addEventListener('click', () => {
            const newState = !hideCompletedSwitch.classList.contains('active');
            hideCompletedSwitch.classList.toggle('active', newState);
            State.set('ui.hideCompleted', newState);
            if (onHideCompletedToggle) {
                onHideCompletedToggle(newState);
            }
        });
    }
    
    // Initialize auto-follow toggle
    const autoFollowSwitch = controls.querySelector('#auto-follow-switch');
    if (autoFollowSwitch) {
        // Load initial state
        const autoFollow = State.get('ui.autoFollow') === true;
        autoFollowSwitch.classList.toggle('active', autoFollow);
        
        autoFollowSwitch.addEventListener('click', () => {
            const newState = !autoFollowSwitch.classList.contains('active');
            autoFollowSwitch.classList.toggle('active', newState);
            State.set('ui.autoFollow', newState);
            if (onAutoFollowToggle) {
                onAutoFollowToggle(newState);
            }
        });
    }
    
    // Initialize follow type selector
    const followTypeSelector = controls.querySelector('#follow-type-selector');
    if (followTypeSelector) {
        // Load initial state
        const followType = State.get('ui.followTypeFilter') || '';
        followTypeSelector.value = followType;
        
        followTypeSelector.addEventListener('change', () => {
            const newType = followTypeSelector.value;
            State.set('ui.followTypeFilter', newType);
            // Optionally trigger a callback if needed
            if (options.onFollowTypeChange) {
                options.onFollowTypeChange(newType);
            }
        });
    }
    
    // Initialize auto-follow config button
    const configBtn = controls.querySelector('#auto-follow-config-btn');
    const configPopover = controls.querySelector('#auto-follow-config-popover');
    
    if (configBtn && configPopover) {
        // Load auto-follow config from state
        let autoFollowConfig = State.get('ui.autoFollowConfig');
        if (!autoFollowConfig) {
            autoFollowConfig = {
                nodeTypes: {
                    task: true,
                    bug: true,
                    idea: false,
                    test: false,
                    doc: false
                }
            };
            State.set('ui.autoFollowConfig', autoFollowConfig);
        }
        
        // Update config toggle states
        const updateConfigToggles = () => {
            const config = State.get('ui.autoFollowConfig');
            if (!config) return;
            
            Object.entries(config.nodeTypes).forEach(([type, enabled]) => {
                const toggle = configPopover.querySelector(`#config-follow-${type}s`);
                if (toggle) {
                    toggle.classList.toggle('active', enabled);
                }
            });
        };
        
        updateConfigToggles();
        
        // Toggle popover visibility
        configBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            configBtn.classList.toggle('active');
            configPopover.classList.toggle('visible');
        });
        
        // Close popover when clicking outside
        document.addEventListener('click', (e) => {
            if (!configPopover.contains(e.target) && e.target !== configBtn) {
                configBtn.classList.remove('active');
                configPopover.classList.remove('visible');
            }
        });
        
        // Handle config toggle clicks
        configPopover.addEventListener('click', (e) => {
            const toggle = e.target.closest('.config-toggle');
            if (toggle) {
                const type = toggle.dataset.type;
                if (type) {
                    const config = State.get('ui.autoFollowConfig');
                    if (config.nodeTypes.hasOwnProperty(type)) {
                        config.nodeTypes[type] = !config.nodeTypes[type];
                        State.set('ui.autoFollowConfig', config);
                        toggle.classList.toggle('active', config.nodeTypes[type]);
                    }
                }
            }
        });
    }
}

/**
 * Mount graph controls to a container
 * @param {HTMLElement} container - Container to mount controls to
 * @param {Object} options - Configuration options (passed to initializeGraphControls)
 * @returns {HTMLElement} The created controls element
 */
export function mountGraphControls(container, options = {}) {
    const controls = createGraphControls();
    container.appendChild(controls);
    initializeGraphControls(controls, options);
    return controls;
}

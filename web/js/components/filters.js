/**
 * Filter Components
 * 
 * Node and edge type filter checkboxes for the sidebar.
 * Persists to localStorage via state management.
 */

import * as State from '../state.js';
import { getEdgeCategoryColor } from '../graph/colors.js';

// Node types for visibility filtering
const NODE_TYPES = {
    task: { name: 'Tasks', emoji: '📋', defaultVisible: true },
    bug: { name: 'Bugs', emoji: '🐛', defaultVisible: true },
    idea: { name: 'Ideas', emoji: '💭', defaultVisible: true },
    test: { name: 'Tests', emoji: '🧪', defaultVisible: true },
    milestone: { name: 'Milestones', emoji: '🏁', defaultVisible: true },
    queue: { name: 'Queue', emoji: '⬡', defaultVisible: true },
    agent: { name: 'Agents', emoji: '🤖', defaultVisible: true },
    doc: { name: 'Docs', emoji: '📄', defaultVisible: true }
};

// Edge types for visibility filtering
const EDGE_TYPES = {
    depends_on: { name: 'Depends', category: 'blocking', defaultVisible: true },
    blocks: { name: 'Blocks', category: 'blocking', defaultVisible: true },
    related_to: { name: 'Related', category: 'informational', defaultVisible: false },
    duplicates: { name: 'Duplicates', category: 'informational', defaultVisible: false },
    fixes: { name: 'Fixes', category: 'fixes', defaultVisible: false },
    caused_by: { name: 'Caused By', category: 'fixes', defaultVisible: false },
    supersedes: { name: 'Supersedes', category: 'informational', defaultVisible: false },
    pinned: { name: 'Pinned', category: 'pinned', defaultVisible: true },
    parent_of: { name: 'Parent', category: 'hierarchy', defaultVisible: true },
    child_of: { name: 'Child', category: 'hierarchy', defaultVisible: true },
    tests: { name: 'Tests', category: 'fixes', defaultVisible: false },
    queued: { name: 'Queued', category: 'queued', defaultVisible: true },
    working_on: { name: 'Working On', category: 'agent', defaultVisible: true },
    worked_on: { name: 'Worked On', category: 'agent', defaultVisible: false },
    documents: { name: 'Documents', category: 'documents', defaultVisible: true },
    impacts: { name: 'Impacts', category: 'impacts', defaultVisible: true }
};

/**
 * Initialize node type filters in the sidebar
 * Creates filter buttons with eye icons for each node type
 */
export function initializeNodeTypeFilters() {
    const container = document.getElementById('sidebar-node-filters');
    if (!container) return;
    
    const currentFilters = State.get('ui.nodeTypeFilters');
    
    // Clear existing content
    container.innerHTML = '';
    
    // Add "All" toggle button first
    const allBtn = document.createElement('button');
    allBtn.className = 'node-filter-btn node-filter-all';
    allBtn.textContent = 'All';
    allBtn.title = 'Toggle all node types';
    
    const updateAllBtnState = () => {
        const allActive = Object.keys(NODE_TYPES).every(type => 
            currentFilters[type] !== false
        );
        allBtn.classList.toggle('active', allActive);
    };
    
    allBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        const allActive = Object.keys(NODE_TYPES).every(type => 
            currentFilters[type] !== false
        );
        const newState = !allActive;
        
        const newFilters = { ...currentFilters };
        for (const type of Object.keys(NODE_TYPES)) {
            newFilters[type] = newState;
        }
        
        State.set('ui.nodeTypeFilters', newFilters);
        
        // Update all visibility button states
        container.querySelectorAll('.node-visibility-btn').forEach(visBtn => {
            const type = visBtn.dataset.nodeType;
            if (type) {
                visBtn.classList.toggle('active', newState);
                visBtn.title = newState 
                    ? `Hide ${NODE_TYPES[type].name}`
                    : `Show ${NODE_TYPES[type].name}`;
            }
        });
        
        // Update all label button states
        container.querySelectorAll('.node-filter-btn[data-node-type]').forEach(btn => {
            btn.classList.toggle('active', newState);
        });
        
        updateAllBtnState();
    });
    
    container.appendChild(allBtn);
    
    // Create filter row for each node type
    for (const [type, info] of Object.entries(NODE_TYPES)) {
        const isActive = currentFilters[type] !== false;
        
        // Create row container: [eye visibility] [emoji label]
        const row = document.createElement('div');
        row.className = 'node-filter-row';
        
        // Create eye visibility button
        const visBtn = document.createElement('button');
        visBtn.className = 'node-visibility-btn' + (isActive ? ' active' : '');
        visBtn.dataset.nodeType = type;
        visBtn.innerHTML = '👁';
        visBtn.title = isActive 
            ? `Hide ${info.name}`
            : `Show ${info.name}`;
        
        // Create label button
        const btn = document.createElement('button');
        btn.className = 'node-filter-btn' + (isActive ? ' active' : '');
        btn.dataset.nodeType = type;
        btn.innerHTML = `${info.emoji} ${info.name}`;
        btn.title = `Toggle ${info.name} visibility`;
        
        // Eye button: toggle visibility
        visBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            const current = State.get('ui.nodeTypeFilters');
            const newFilters = { ...current };
            newFilters[type] = !current[type];
            
            State.set('ui.nodeTypeFilters', newFilters);
            
            visBtn.classList.toggle('active', newFilters[type]);
            btn.classList.toggle('active', newFilters[type]);
            visBtn.title = newFilters[type]
                ? `Hide ${info.name}`
                : `Show ${info.name}`;
            updateAllBtnState();
        });
        
        // Label button: also toggles visibility
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            const current = State.get('ui.nodeTypeFilters');
            const newFilters = { ...current };
            newFilters[type] = !current[type];
            
            State.set('ui.nodeTypeFilters', newFilters);
            
            visBtn.classList.toggle('active', newFilters[type]);
            btn.classList.toggle('active', newFilters[type]);
            visBtn.title = newFilters[type]
                ? `Hide ${info.name}`
                : `Show ${info.name}`;
            updateAllBtnState();
        });
        
        row.appendChild(visBtn);
        row.appendChild(btn);
        container.appendChild(row);
    }
    
    updateAllBtnState();
}

/**
 * Initialize edge type filters in the sidebar
 * Creates filter buttons with visibility and color indicators for each edge type
 */
export function initializeEdgeTypeFilters() {
    const container = document.getElementById('sidebar-edge-filters');
    if (!container) return;
    
    const currentFilters = State.get('ui.edgeTypeFilters');
    
    // Clear existing content
    container.innerHTML = '';
    
    // Add "All" toggle button first
    const allBtn = document.createElement('button');
    allBtn.className = 'edge-filter-btn edge-filter-all';
    allBtn.textContent = 'All';
    allBtn.title = 'Toggle all edge types';
    
    const updateAllBtnState = () => {
        const allActive = Object.keys(EDGE_TYPES).every(type => 
            currentFilters[type] !== false
        );
        allBtn.classList.toggle('active', allActive);
    };
    
    allBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        const allActive = Object.keys(EDGE_TYPES).every(type => 
            currentFilters[type] !== false
        );
        const newState = !allActive;
        
        const newFilters = { ...currentFilters };
        for (const type of Object.keys(EDGE_TYPES)) {
            newFilters[type] = newState;
        }
        
        State.set('ui.edgeTypeFilters', newFilters);
        
        // Update all visibility button states
        container.querySelectorAll('.edge-visibility-btn').forEach(visBtn => {
            const type = visBtn.dataset.edgeType;
            if (type) {
                visBtn.classList.toggle('active', newState);
                visBtn.title = newState 
                    ? `Hide ${EDGE_TYPES[type].name} edges`
                    : `Show ${EDGE_TYPES[type].name} edges`;
            }
        });
        
        // Update all label button states
        container.querySelectorAll('.edge-filter-btn[data-edge-type]').forEach(btn => {
            btn.classList.toggle('active', newState);
        });
        
        updateAllBtnState();
    });
    
    container.appendChild(allBtn);
    
    // Create filter row for each edge type
    for (const [type, info] of Object.entries(EDGE_TYPES)) {
        const isActive = currentFilters[type] !== false;
        
        // Create row container: [eye visibility] [color label]
        const row = document.createElement('div');
        row.className = 'edge-filter-row';
        
        // Create eye visibility button
        const visBtn = document.createElement('button');
        visBtn.className = 'edge-visibility-btn' + (isActive ? ' active' : '');
        visBtn.dataset.edgeType = type;
        visBtn.innerHTML = '👁';
        visBtn.title = isActive 
            ? `Hide ${info.name} edges`
            : `Show ${info.name} edges`;
        
        // Create color label button
        const btn = document.createElement('button');
        btn.className = 'edge-filter-btn' + (isActive ? ' active' : '');
        btn.dataset.edgeType = type;
        btn.title = `Toggle ${info.name} visibility`;
        
        const dot = document.createElement('span');
        dot.className = 'edge-filter-dot';
        dot.style.backgroundColor = getEdgeCategoryColor(info.category);
        
        btn.appendChild(dot);
        btn.appendChild(document.createTextNode(info.name));
        
        // Eye button: toggle visibility
        visBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            const current = State.get('ui.edgeTypeFilters');
            const newFilters = { ...current };
            newFilters[type] = !current[type];
            
            State.set('ui.edgeTypeFilters', newFilters);
            
            visBtn.classList.toggle('active', newFilters[type]);
            btn.classList.toggle('active', newFilters[type]);
            visBtn.title = newFilters[type]
                ? `Hide ${info.name} edges`
                : `Show ${info.name} edges`;
            updateAllBtnState();
        });
        
        // Label button: also toggles visibility
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            const current = State.get('ui.edgeTypeFilters');
            const newFilters = { ...current };
            newFilters[type] = !current[type];
            
            State.set('ui.edgeTypeFilters', newFilters);
            
            visBtn.classList.toggle('active', newFilters[type]);
            btn.classList.toggle('active', newFilters[type]);
            visBtn.title = newFilters[type]
                ? `Hide ${info.name} edges`
                : `Show ${info.name} edges`;
            updateAllBtnState();
        });
        
        row.appendChild(visBtn);
        row.appendChild(btn);
        container.appendChild(row);
    }
    
    updateAllBtnState();
}

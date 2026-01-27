/**
 * Selection Context Gathering
 * 
 * Utilities for gathering context about selected entities for AI agent consumption.
 * Used by the summarize feature to provide agents with comprehensive data about
 * the user's selection.
 */

import { getSelectedNodes, getNode } from '../state.js';
import { getEdges, get } from '../state.js';

/**
 * Gather comprehensive context about currently selected entities
 * 
 * Collects:
 * - Full entity data for each selected node
 * - Relationships (edges) between selected nodes
 * - Relationships to external (non-selected) nodes
 * - Recent activity/log entries related to selected entities
 * 
 * @param {Object} options - Options for context gathering
 * @param {boolean} options.includeExternalEdges - Include edges to non-selected nodes (default: true)
 * @param {number} options.maxLogEntries - Max log entries per entity (default: 10)
 * @returns {Object} Selection context formatted for agent consumption
 */
export function gatherSelectionContext(options = {}) {
    const {
        includeExternalEdges = true,
        maxLogEntries = 10
    } = options;
    
    const selectedNodeIds = getSelectedNodes();
    
    // If nothing selected, return empty context
    if (selectedNodeIds.length === 0) {
        return {
            selectionCount: 0,
            entities: [],
            internalEdges: [],
            externalEdges: [],
            recentActivity: [],
            summary: 'No entities selected'
        };
    }
    
    // Gather full entity data for each selected node
    const entities = [];
    const entitiesById = new Map();
    
    for (const nodeId of selectedNodeIds) {
        const entity = getNode(nodeId);
        if (entity) {
            entities.push(entity);
            entitiesById.set(nodeId, entity);
        }
    }
    
    // Gather edges
    const allEdges = getEdges();
    const internalEdges = []; // Edges between selected nodes
    const externalEdges = []; // Edges from selected to non-selected
    
    for (const edge of allEdges) {
        const sourceSelected = selectedNodeIds.includes(edge.source);
        const targetSelected = selectedNodeIds.includes(edge.target);
        
        if (sourceSelected && targetSelected) {
            // Internal edge: both nodes selected
            internalEdges.push({
                source: edge.source,
                target: edge.target,
                type: edge.type,
                label: edge.label || edge.type
            });
        } else if (includeExternalEdges && (sourceSelected || targetSelected)) {
            // External edge: one node selected, one not
            const externalNodeId = sourceSelected ? edge.target : edge.source;
            const selectedNodeId = sourceSelected ? edge.source : edge.target;
            const direction = sourceSelected ? 'outbound' : 'inbound';
            
            // Get external node data
            const externalNode = getNode(externalNodeId);
            
            externalEdges.push({
                selectedNode: selectedNodeId,
                externalNode: externalNodeId,
                externalNodeType: externalNode?.type,
                externalNodeTitle: externalNode?.title || externalNode?.name,
                externalNodeStatus: externalNode?.status,
                direction,
                type: edge.type,
                label: edge.label || edge.type
            });
        }
    }
    
    // Gather recent activity (log entries)
    const logEntries = get('log') || [];
    const recentActivity = [];
    
    // Filter log entries related to selected entities
    for (const entry of logEntries) {
        // Check if this log entry is about any selected entity
        const relatedToSelection = selectedNodeIds.some(nodeId => {
            return entry.entity_id === nodeId || 
                   entry.target_id === nodeId ||
                   (entry.message && entry.message.includes(nodeId));
        });
        
        if (relatedToSelection) {
            recentActivity.push({
                timestamp: entry.timestamp,
                entityId: entry.entity_id,
                action: entry.action,
                field: entry.field,
                oldValue: entry.old_value,
                newValue: entry.new_value,
                message: entry.message,
                actor: entry.actor || entry.user
            });
            
            // Limit entries per entity
            if (recentActivity.length >= selectedNodeIds.length * maxLogEntries) {
                break;
            }
        }
    }
    
    // Sort activity by timestamp (most recent first)
    recentActivity.sort((a, b) => {
        const timeA = new Date(a.timestamp).getTime();
        const timeB = new Date(b.timestamp).getTime();
        return timeB - timeA;
    });
    
    // Build summary
    const entityTypes = {};
    for (const entity of entities) {
        const type = entity.type;
        entityTypes[type] = (entityTypes[type] || 0) + 1;
    }
    
    const typeSummary = Object.entries(entityTypes)
        .map(([type, count]) => `${count} ${type}${count > 1 ? 's' : ''}`)
        .join(', ');
    
    const summary = `Selected ${entities.length} entities (${typeSummary}), ` +
                   `${internalEdges.length} internal relationships, ` +
                   `${externalEdges.length} external connections, ` +
                   `${recentActivity.length} recent activity entries`;
    
    return {
        selectionCount: entities.length,
        entities,
        internalEdges,
        externalEdges,
        recentActivity: recentActivity.slice(0, maxLogEntries * selectedNodeIds.length),
        summary,
        metadata: {
            timestamp: new Date().toISOString(),
            entityTypes,
            internalEdgeCount: internalEdges.length,
            externalEdgeCount: externalEdges.length,
            activityCount: recentActivity.length
        }
    };
}

/**
 * Format selection context as markdown for agent consumption
 * 
 * @param {Object} context - Context object from gatherSelectionContext()
 * @returns {string} Markdown-formatted context
 */
export function formatContextAsMarkdown(context) {
    if (context.selectionCount === 0) {
        return '# Selection Context\n\nNo entities selected.';
    }
    
    let md = '# Selection Context\n\n';
    md += `**Summary:** ${context.summary}\n\n`;
    
    // Entities section
    md += '## Selected Entities\n\n';
    for (const entity of context.entities) {
        md += `### ${entity.id}: ${entity.title || entity.name}\n\n`;
        md += `- **Type:** ${entity.type}\n`;
        if (entity.status) md += `- **Status:** ${entity.status}\n`;
        if (entity.priority !== undefined) md += `- **Priority:** ${entity.priority}\n`;
        if (entity.description) md += `- **Description:** ${entity.description}\n`;
        if (entity.short_name) md += `- **Short name:** ${entity.short_name}\n`;
        if (entity.tags && entity.tags.length > 0) md += `- **Tags:** ${entity.tags.join(', ')}\n`;
        md += '\n';
    }
    
    // Internal relationships
    if (context.internalEdges.length > 0) {
        md += '## Internal Relationships\n\n';
        md += 'Relationships between selected entities:\n\n';
        for (const edge of context.internalEdges) {
            md += `- \`${edge.source}\` → \`${edge.target}\` (${edge.type})\n`;
        }
        md += '\n';
    }
    
    // External connections
    if (context.externalEdges.length > 0) {
        md += '## External Connections\n\n';
        md += 'Connections to non-selected entities:\n\n';
        
        // Group by selected node
        const byNode = {};
        for (const edge of context.externalEdges) {
            if (!byNode[edge.selectedNode]) {
                byNode[edge.selectedNode] = [];
            }
            byNode[edge.selectedNode].push(edge);
        }
        
        for (const [nodeId, edges] of Object.entries(byNode)) {
            md += `### ${nodeId}\n\n`;
            for (const edge of edges) {
                const direction = edge.direction === 'outbound' ? '→' : '←';
                md += `- ${direction} \`${edge.externalNode}\` (${edge.externalNodeType})`;
                if (edge.externalNodeTitle) md += `: ${edge.externalNodeTitle}`;
                if (edge.externalNodeStatus) md += ` [${edge.externalNodeStatus}]`;
                md += ` - ${edge.type}\n`;
            }
            md += '\n';
        }
    }
    
    // Recent activity
    if (context.recentActivity.length > 0) {
        md += '## Recent Activity\n\n';
        for (const activity of context.recentActivity) {
            const timestamp = new Date(activity.timestamp).toLocaleString();
            md += `- **${timestamp}** - ${activity.entityId}: ${activity.action}`;
            if (activity.field) md += ` (${activity.field})`;
            if (activity.actor) md += ` by ${activity.actor}`;
            md += '\n';
            if (activity.message) md += `  ${activity.message}\n`;
        }
        md += '\n';
    }
    
    return md;
}

/**
 * Format selection context as JSON for API transmission
 * 
 * @param {Object} context - Context object from gatherSelectionContext()
 * @returns {string} JSON string
 */
export function formatContextAsJSON(context) {
    return JSON.stringify(context, null, 2);
}

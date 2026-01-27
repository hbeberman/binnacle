/**
 * Tests for graph-controls.js
 */

import { createGraphControls, initializeGraphControls } from './graph-controls.js';
import * as State from '../state.js';

describe('Graph Controls', () => {
    let container;
    
    beforeEach(() => {
        // Reset state before each test
        State.reset();
        
        // Create a container for the controls
        container = document.createElement('div');
        document.body.appendChild(container);
    });
    
    afterEach(() => {
        // Clean up
        if (container && container.parentNode) {
            container.parentNode.removeChild(container);
        }
    });
    
    describe('Agent Dropdown', () => {
        test('clicking agent item sets selectedNode', () => {
            // Create and mount controls
            const controls = createGraphControls();
            container.appendChild(controls);
            initializeGraphControls(controls);
            
            // Mock agent data
            const mockAgent = {
                id: 'bn-agent-123',
                type: 'agent',
                status: 'active',
                title: 'Test Agent',
                _agent: {
                    name: 'Test Agent',
                    purpose: 'Testing'
                }
            };
            
            // Set agents in state to trigger list update
            State.set('entities.agents', [mockAgent]);
            
            // Wait for async updates
            setTimeout(() => {
                const agentList = controls.querySelector('#graph-agent-list');
                expect(agentList).toBeTruthy();
                
                const agentItem = agentList.querySelector('.agent-item');
                expect(agentItem).toBeTruthy();
                
                // Click the agent item
                agentItem.click();
                
                // Verify state was updated correctly
                expect(State.get('ui.selectedNode')).toBe('bn-agent-123');
                expect(State.get('ui.autoFollow')).toBe(true);
                expect(State.get('ui.followTypeFilter')).toBe('agent');
            }, 100);
        });
        
        test('clicking agent enables follow mode even when disabled', () => {
            // Create and mount controls
            const controls = createGraphControls();
            container.appendChild(controls);
            initializeGraphControls(controls);
            
            // Disable follow mode initially
            State.set('ui.autoFollow', false);
            State.set('ui.followTypeFilter', '');
            
            // Mock agent data
            const mockAgent = {
                id: 'bn-agent-456',
                type: 'agent',
                status: 'idle',
                title: 'Another Agent'
            };
            
            State.set('entities.agents', [mockAgent]);
            
            setTimeout(() => {
                const agentItem = controls.querySelector('.agent-item');
                expect(agentItem).toBeTruthy();
                
                // Click the agent
                agentItem.click();
                
                // Verify follow mode was enabled and set to agent
                expect(State.get('ui.autoFollow')).toBe(true);
                expect(State.get('ui.followTypeFilter')).toBe('agent');
                expect(State.get('ui.selectedNode')).toBe('bn-agent-456');
            }, 100);
        });
        
        test('clicking different agents switches followTypeFilter to agent', () => {
            const controls = createGraphControls();
            container.appendChild(controls);
            initializeGraphControls(controls);
            
            // Set follow mode to tasks initially
            State.set('ui.autoFollow', true);
            State.set('ui.followTypeFilter', 'task');
            
            const mockAgent = {
                id: 'bn-agent-789',
                type: 'agent',
                status: 'active',
                title: 'Third Agent'
            };
            
            State.set('entities.agents', [mockAgent]);
            
            setTimeout(() => {
                const agentItem = controls.querySelector('.agent-item');
                agentItem.click();
                
                // Should switch from 'task' to 'agent'
                expect(State.get('ui.followTypeFilter')).toBe('agent');
                expect(State.get('ui.autoFollow')).toBe(true);
            }, 100);
        });
    });
});

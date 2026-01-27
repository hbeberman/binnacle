/**
 * Markdown Parser using marked library
 * 
 * Supports:
 * - All standard Markdown features
 * - GFM (GitHub Flavored Markdown): tables, task lists, strikethrough
 * - Code syntax highlighting via highlight.js
 */

import { marked } from 'marked';
import hljs from 'highlight.js';

// Configure marked with GFM and custom renderer
const renderer = new marked.Renderer();

// Override link rendering to add target="_blank" and rel="noopener" for security
renderer.link = function(href, title, text) {
    // Build the link with attributes in the expected order
    const titleAttr = title ? ` title="${title}"` : '';
    return `<a href="${href}"${titleAttr} target="_blank" rel="noopener">${text}</a>`;
};

// Override code block rendering to add syntax highlighting
renderer.code = function(code, language) {
    if (language && hljs.getLanguage(language)) {
        try {
            const highlighted = hljs.highlight(code, { language: language }).value;
            return `<pre><code class="language-${language}">${highlighted}</code></pre>`;
        } catch (err) {
            console.error('Syntax highlighting error:', err);
        }
    }
    // No highlighting if language not recognized
    const escaped = code.replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
    return `<pre><code${language ? ` class="language-${language}"` : ''}>${escaped}</code></pre>`;
};

/**
 * Regex to match binnacle entity IDs
 */
const ENTITY_ID_PATTERN = /\b(bn[a-z]?-[a-f0-9]{4})\b/gi;

/**
 * Linkify entity IDs in HTML content
 * Converts entity IDs (bn-xxxx, bnt-xxxx, etc.) to clickable spans
 * Skips entity IDs inside code blocks and inline code
 * @param {string} html - HTML content
 * @returns {string} HTML with linkified entity IDs
 */
function linkifyEntityIds(html) {
    if (!html) return html;
    
    // More sophisticated approach: parse HTML to avoid linkifying inside <code> and <pre> tags
    const codeBlockPattern = /(<(?:code|pre)[^>]*>)([\s\S]*?)(<\/(?:code|pre)>)/gi;
    const segments = [];
    let lastIndex = 0;
    let match;
    
    // First, identify all code blocks and preserve them
    codeBlockPattern.lastIndex = 0;
    while ((match = codeBlockPattern.exec(html)) !== null) {
        // Add text before code block (linkify it)
        const beforeCode = html.slice(lastIndex, match.index);
        segments.push({ text: beforeCode, isCode: false });
        
        // Add the code block (don't linkify)
        segments.push({ text: match[0], isCode: true });
        
        lastIndex = codeBlockPattern.lastIndex;
    }
    
    // Add remaining text after last code block
    if (lastIndex < html.length) {
        segments.push({ text: html.slice(lastIndex), isCode: false });
    }
    
    // If no code blocks were found, just process the whole string
    if (segments.length === 0) {
        segments.push({ text: html, isCode: false });
    }
    
    // Process each segment
    return segments.map(segment => {
        if (segment.isCode) {
            return segment.text;
        }
        
        // Linkify entity IDs in non-code segments
        ENTITY_ID_PATTERN.lastIndex = 0;
        return segment.text.replace(ENTITY_ID_PATTERN, (match) => {
            return `<span class="clickable-entity-id" data-entity-id="${match}" title="Click to view ${match}">${match}</span>`;
        });
    }).join('');
}

// Configure marked with all options in one place
marked.use({
    gfm: true,              // Enable GitHub Flavored Markdown
    breaks: false,          // Don't convert \n to <br> (GFM default)
    pedantic: false,        // Relaxed parsing
    renderer: renderer,
    // Hooks to post-process the output
    hooks: {
        postprocess(html) {
            // Remove trailing newlines in code blocks but preserve paragraph spacing
            let processed = html
                .replace(/(<code[^>]*>)([\s\S]*?)\n(<\/code>)/g, '$1$2$3')  // Remove trailing \n before </code>
                .replace(/(<\/p>)\n(<p>)/g, '$1\n\n$2')                      // Double newline between paragraphs
                .replace(/(<\/[uo]l>)\n(<p>)/g, '$1\n\n$2')                 // Double newline between list and paragraph
                .replace(/(<\/h[1-6]>)\n(<p>)/g, '$1\n\n$2')                // Double newline between heading and paragraph
                .replace(/(<\/p>)\n(<h[1-6]>)/g, '$1\n\n$2')                // Double newline between paragraph and heading
                .replace(/(<\/h[1-6]>)\n(<h[1-6]>)/g, '$1\n\n$2')           // Double newline between headings
                .trim();                                                      // Remove leading/trailing whitespace
            
            // Linkify entity IDs
            return linkifyEntityIds(processed);
        }
    },
    // Sanitize HTML in walkTokens
    walkTokens(token) {
        // Sanitize any HTML in tokens
        if (token.type === 'html') {
            // Escape HTML tags to prevent XSS
            token.text = token.text
                .replace(/&/g, '&amp;')
                .replace(/</g, '&lt;')
                .replace(/>/g, '&gt;')
                .replace(/"/g, '&quot;')
                .replace(/'/g, '&#39;');
        }
    }
});

/**
 * Parse markdown to HTML
 * @param {string} markdown - Markdown text
 * @returns {string} Rendered HTML
 */
export function parseMarkdown(markdown) {
    if (!markdown) return '';
    
    try {
        return marked.parse(markdown);
    } catch (err) {
        console.error('Markdown parsing error:', err);
        return '';
    }
}

/**
 * Render markdown content to a DOM element
 * @param {HTMLElement} element - Target element
 * @param {string} markdown - Markdown content
 */
export function renderMarkdown(element, markdown) {
    if (!element) {
        console.error('Target element is required for markdown rendering');
        return;
    }
    
    element.innerHTML = parseMarkdown(markdown);
}

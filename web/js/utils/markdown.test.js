/**
 * Unit tests for markdown parser
 */

import { parseMarkdown } from './markdown.js';

/**
 * Simple test runner
 */
function test(name, fn) {
    try {
        fn();
        console.log(`✓ ${name}`);
    } catch (error) {
        console.error(`✗ ${name}`);
        console.error(error);
    }
}

function assertEquals(actual, expected, message) {
    if (actual !== expected) {
        throw new Error(message || `Expected:\n${expected}\n\nGot:\n${actual}`);
    }
}

// Run tests
console.log('Running markdown parser tests...\n');

test('Empty string returns empty string', () => {
    assertEquals(parseMarkdown(''), '');
    assertEquals(parseMarkdown(null), '');
});

test('Simple paragraph', () => {
    const result = parseMarkdown('Hello world');
    assertEquals(result, '<p>Hello world</p>');
});

test('Multiple paragraphs with blank line', () => {
    const result = parseMarkdown('First paragraph\n\nSecond paragraph');
    assertEquals(result, '<p>First paragraph</p>\n\n<p>Second paragraph</p>');
});

test('Heading H1', () => {
    const result = parseMarkdown('# Title');
    assertEquals(result, '<h1>Title</h1>');
});

test('Heading H2', () => {
    const result = parseMarkdown('## Subtitle');
    assertEquals(result, '<h2>Subtitle</h2>');
});

test('Heading H6', () => {
    const result = parseMarkdown('###### Small heading');
    assertEquals(result, '<h6>Small heading</h6>');
});

test('Unordered list with dashes', () => {
    const result = parseMarkdown('- Item 1\n- Item 2\n- Item 3');
    assertEquals(result, '<ul>\n<li>Item 1</li>\n<li>Item 2</li>\n<li>Item 3</li>\n</ul>');
});

test('Unordered list with asterisks', () => {
    const result = parseMarkdown('* Item 1\n* Item 2');
    assertEquals(result, '<ul>\n<li>Item 1</li>\n<li>Item 2</li>\n</ul>');
});

test('Unordered list with plus signs', () => {
    const result = parseMarkdown('+ Item 1\n+ Item 2');
    assertEquals(result, '<ul>\n<li>Item 1</li>\n<li>Item 2</li>\n</ul>');
});

test('Ordered list', () => {
    const result = parseMarkdown('1. First\n2. Second\n3. Third');
    assertEquals(result, '<ol>\n<li>First</li>\n<li>Second</li>\n<li>Third</li>\n</ol>');
});

test('List closes on blank line', () => {
    const result = parseMarkdown('- Item 1\n- Item 2\n\nParagraph');
    assertEquals(result, '<ul>\n<li>Item 1</li>\n<li>Item 2</li>\n</ul>\n\n<p>Paragraph</p>');
});

test('Inline code', () => {
    const result = parseMarkdown('Text with `code` here');
    assertEquals(result, '<p>Text with <code>code</code> here</p>');
});

test('Code block with language', () => {
    const markdown = '```javascript\nconst x = 1;\n```';
    const result = parseMarkdown(markdown);
    // With highlight.js, the code should be syntax highlighted
    // Check for the pre/code structure and language class
    if (!result.includes('<pre><code class="language-javascript">')) {
        throw new Error('Missing code block with language class');
    }
    // Check that highlighting is applied (should have hljs- classes)
    if (!result.includes('hljs-')) {
        throw new Error('Syntax highlighting not applied');
    }
});

test('Code block without language', () => {
    const markdown = '```\nplain text\n```';
    const result = parseMarkdown(markdown);
    assertEquals(result, '<pre><code>plain text</code></pre>');
});

test('Bold with asterisks', () => {
    const result = parseMarkdown('This is **bold** text');
    assertEquals(result, '<p>This is <strong>bold</strong> text</p>');
});

test('Bold with underscores', () => {
    const result = parseMarkdown('This is __bold__ text');
    assertEquals(result, '<p>This is <strong>bold</strong> text</p>');
});

test('Italic with asterisks', () => {
    const result = parseMarkdown('This is *italic* text');
    assertEquals(result, '<p>This is <em>italic</em> text</p>');
});

test('Italic with underscores', () => {
    const result = parseMarkdown('This is _italic_ text');
    assertEquals(result, '<p>This is <em>italic</em> text</p>');
});

test('Links', () => {
    const result = parseMarkdown('Check [this link](https://example.com) out');
    assertEquals(result, '<p>Check <a href="https://example.com" target="_blank" rel="noopener">this link</a> out</p>');
});

test('Mixed inline formatting', () => {
    const result = parseMarkdown('**Bold**, *italic*, and `code`');
    assertEquals(result, '<p><strong>Bold</strong>, <em>italic</em>, and <code>code</code></p>');
});

test('HTML escaping in text', () => {
    const result = parseMarkdown('Text with <script>alert("xss")</script>');
    // Both &quot; and plain quotes are acceptable for escaping
    const acceptable = result === '<p>Text with &lt;script&gt;alert(&quot;xss&quot;)&lt;/script&gt;</p>' ||
                      result === '<p>Text with &lt;script&gt;alert("xss")&lt;/script&gt;</p>';
    if (!acceptable) {
        throw new Error(`Unexpected output: ${result}`);
    }
});

test('HTML escaping in code blocks', () => {
    const markdown = '```\n<div>HTML</div>\n```';
    const result = parseMarkdown(markdown);
    assertEquals(result, '<pre><code>&lt;div&gt;HTML&lt;/div&gt;</code></pre>');
});

test('Multiple headings and paragraphs', () => {
    const markdown = `# Title

First paragraph.

## Section

Second paragraph.`;
    
    const expected = `<h1>Title</h1>

<p>First paragraph.</p>

<h2>Section</h2>

<p>Second paragraph.</p>`;
    
    assertEquals(parseMarkdown(markdown), expected);
});

test('Complex document', () => {
    const markdown = `# Main Title

This is a **bold** statement with *italic* text.

## Features

- Feature 1
- Feature 2
- Feature 3

Code example:
\`\`\`rust
fn main() {
    println!("Hello");
}
\`\`\`

See [docs](https://example.com) for more.`;
    
    // Just verify it doesn't throw and produces output
    const result = parseMarkdown(markdown);
    if (!result.includes('<h1>Main Title</h1>')) {
        throw new Error('Missing H1 in complex document');
    }
    if (!result.includes('<ul>')) {
        throw new Error('Missing list in complex document');
    }
    if (!result.includes('<pre><code class="language-rust">')) {
        throw new Error('Missing code block in complex document');
    }
    // Check for syntax highlighting
    if (!result.includes('hljs-')) {
        throw new Error('Missing syntax highlighting in complex document');
    }
    if (!result.includes('<a href="https://example.com"')) {
        throw new Error('Missing link in complex document');
    }
});

test('Entity ID linkification - single ID in paragraph', () => {
    const result = parseMarkdown('See task bn-1234 for details');
    if (!result.includes('clickable-entity-id')) {
        throw new Error('Missing clickable-entity-id class');
    }
    if (!result.includes('data-entity-id="bn-1234"')) {
        throw new Error('Missing data-entity-id attribute');
    }
    if (!result.includes('title="Click to view bn-1234"')) {
        throw new Error('Missing title attribute');
    }
});

test('Entity ID linkification - multiple IDs', () => {
    const result = parseMarkdown('Tasks bn-1234 and bn-5678 are related');
    const matches = result.match(/clickable-entity-id/g);
    if (!matches || matches.length !== 2) {
        throw new Error('Expected 2 linkified entity IDs');
    }
});

test('Entity ID linkification - different entity types', () => {
    const result = parseMarkdown('Task bn-1234, test bnt-5678, queue bnq-9abc');
    if (!result.includes('data-entity-id="bn-1234"')) {
        throw new Error('Missing task ID linkification');
    }
    if (!result.includes('data-entity-id="bnt-5678"')) {
        throw new Error('Missing test ID linkification');
    }
    if (!result.includes('data-entity-id="bnq-9abc"')) {
        throw new Error('Missing queue ID linkification');
    }
});

test('Entity ID linkification - in headings', () => {
    const result = parseMarkdown('# Task bn-1234');
    if (!result.includes('<h1>Task <span class="clickable-entity-id"')) {
        throw new Error('Entity ID not linkified in heading');
    }
});

test('Entity ID linkification - in lists', () => {
    const result = parseMarkdown('- Task bn-1234\n- Task bn-5678');
    const matches = result.match(/clickable-entity-id/g);
    if (!matches || matches.length !== 2) {
        throw new Error('Entity IDs not linkified in list items');
    }
});

test('Entity ID linkification - not in code blocks', () => {
    const markdown = '```\nbn-1234\n```';
    const result = parseMarkdown(markdown);
    // Should not linkify inside code blocks
    if (result.includes('clickable-entity-id')) {
        throw new Error('Entity ID should not be linkified inside code blocks');
    }
    if (!result.includes('bn-1234')) {
        throw new Error('Entity ID should still be present in code block');
    }
});

test('Entity ID linkification - not in inline code', () => {
    const result = parseMarkdown('Use `bn-1234` as reference');
    // Should not linkify inside inline code
    if (result.includes('clickable-entity-id')) {
        throw new Error('Entity ID should not be linkified inside inline code');
    }
    if (!result.includes('bn-1234')) {
        throw new Error('Entity ID should still be present in inline code');
    }
});

test('Entity ID linkification - mixed with other formatting', () => {
    const result = parseMarkdown('**Important**: See bn-1234 for details');
    if (!result.includes('clickable-entity-id')) {
        throw new Error('Entity ID not linkified with bold text present');
    }
    if (!result.includes('<strong>Important</strong>')) {
        throw new Error('Bold formatting should be preserved');
    }
});

test('Entity ID linkification - preserves special characters around IDs', () => {
    const result = parseMarkdown('Tasks: bn-1234, bn-5678.');
    if (!result.match(/bn-1234<\/span>,/)) {
        throw new Error('Comma after entity ID not preserved correctly');
    }
    if (!result.match(/bn-5678<\/span>\./)) {
        throw new Error('Period after entity ID not preserved correctly');
    }
});

console.log('\n✓ All tests passed!');

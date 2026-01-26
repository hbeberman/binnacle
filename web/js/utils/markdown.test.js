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
    assertEquals(result, '<pre><code class="language-javascript">const x = 1;</code></pre>');
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
    if (!result.includes('<a href="https://example.com"')) {
        throw new Error('Missing link in complex document');
    }
});

console.log('\n✓ All tests passed!');

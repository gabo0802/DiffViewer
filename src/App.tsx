import React from 'react'
import Editor from '@monaco-editor/react'
import './styles/global.css'

export default function App() {
  return (
    <div className="app-root">
      <aside className="sidebar">Workspace / DiffSets</aside>
      <main className="main">
        <div className="toolbar">Toolbar — Open a diff to begin</div>
        <div className="editor-row">
          <div className="editor-col">
            <div className="editor-label">Left</div>
            <Editor height="60vh" defaultLanguage="text" defaultValue={"// left file"} />
          </div>
          <div className="editor-col">
            <div className="editor-label">Right</div>
            <Editor height="60vh" defaultLanguage="text" defaultValue={"// right file"} />
          </div>
        </div>
      </main>
    </div>
  )
}

import { useState, useEffect } from 'react';
import { api } from '../api';
import { open } from '@tauri-apps/plugin-dialog';

interface PaperSection {
  id: string;
  heading: string;
  level: number;
  content: string;
  token_estimate: number;
}

interface ParsedPaper {
  title: string | null;
  authors: string[];
  abstract_text: string | null;
  sections: PaperSection[];
  unassigned_content: string;
  metadata: {
    doi: string | null;
    arxiv_id: string | null;
    year: number | null;
  };
  parsing_warnings: string[];
}

interface ImportTrainingDataModalProps {
  isOpen: boolean;
  onClose: () => void;
  projectId: string;
  localModelId?: string;
  onImportComplete: () => void;
  /** Called when import starts - parent can show status. If provided, modal closes and parent runs the import. */
  onImportRequest?: (runImport: (onProgress?: (msg: string) => void) => Promise<void>) => void;
}

export function ImportTrainingDataModal({
  isOpen,
  onClose,
  projectId,
  localModelId,
  onImportComplete,
  onImportRequest,
}: ImportTrainingDataModalProps) {
  const [importType, setImportType] = useState<'file' | 'folder' | 'url' | 'text' | 'coder_history' | 'profile_chat' | 'research_paper'>('file');
  const [filePath, setFilePath] = useState('');
  const [folderPath, setFolderPath] = useState('');
  const [workspacePath, setWorkspacePath] = useState('');
  const [profiles, setProfiles] = useState<any[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string>('');
  const [includeSubfolders, setIncludeSubfolders] = useState(true);
  const [url, setUrl] = useState('');
  const [textContent, setTextContent] = useState('');
  const [format, setFormat] = useState('auto');
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);
  
  // Research paper state
  const [parsedPaper, setParsedPaper] = useState<ParsedPaper | null>(null);
  const [parsingPaper, setParsingPaper] = useState(false);
  const [selectedSections, setSelectedSections] = useState<string[]>([]);
  const [includeAbstract, setIncludeAbstract] = useState(true);
  const [includeUnassigned, setIncludeUnassigned] = useState(false);
  const [chunkBySection, setChunkBySection] = useState(true);
  const [paperPreset, setPaperPreset] = useState<'full' | 'abstract_conclusions' | 'methods_results' | 'custom'>('full');
  const [paperFolderPath, setPaperFolderPath] = useState('');
  const [paperPdfPaths, setPaperPdfPaths] = useState<string[]>([]);
  const [paperFolderRecursive, setPaperFolderRecursive] = useState(true);

  useEffect(() => {
    if (isOpen && importType === 'coder_history') {
      api.getWorkspacePath().then(setWorkspacePath).catch(() => setWorkspacePath(''));
    }
  }, [isOpen, importType]);

  useEffect(() => {
    if (isOpen && importType === 'profile_chat') {
      api.listProfiles().then(setProfiles).catch(() => setProfiles([]));
    }
  }, [isOpen, importType]);

  // Reset paper state when switching away from research_paper mode
  useEffect(() => {
    if (importType !== 'research_paper') {
      setParsedPaper(null);
      setSelectedSections([]);
      setPaperFolderPath('');
      setPaperPdfPaths([]);
    }
  }, [importType]);

  // Apply paper presets
  useEffect(() => {
    if (!parsedPaper) return;
    
    const allSectionIds = parsedPaper.sections.map(s => s.id);
    
    switch (paperPreset) {
      case 'full':
        setSelectedSections(allSectionIds);
        setIncludeAbstract(true);
        setIncludeUnassigned(true);
        break;
      case 'abstract_conclusions':
        setSelectedSections(
          parsedPaper.sections
            .filter(s => s.heading.toLowerCase().includes('conclusion'))
            .map(s => s.id)
        );
        setIncludeAbstract(true);
        setIncludeUnassigned(false);
        break;
      case 'methods_results':
        setSelectedSections(
          parsedPaper.sections
            .filter(s => 
              s.heading.toLowerCase().includes('method') ||
              s.heading.toLowerCase().includes('result') ||
              s.heading.toLowerCase().includes('experiment')
            )
            .map(s => s.id)
        );
        setIncludeAbstract(false);
        setIncludeUnassigned(false);
        break;
      case 'custom':
        // Don't change selections in custom mode
        break;
    }
  }, [paperPreset, parsedPaper]);

  const getImportOptionsFromPreset = (paper: ParsedPaper) => {
    const allSectionIds = paper.sections.map((s: PaperSection) => s.id);
    switch (paperPreset) {
      case 'full':
        return { include_sections: allSectionIds, include_abstract: true, include_unassigned: true };
      case 'abstract_conclusions':
        return {
          include_sections: paper.sections.filter((s: PaperSection) => s.heading.toLowerCase().includes('conclusion')).map((s: PaperSection) => s.id),
          include_abstract: true,
          include_unassigned: false,
        };
      case 'methods_results':
        return {
          include_sections: paper.sections.filter((s: PaperSection) =>
            s.heading.toLowerCase().includes('method') || s.heading.toLowerCase().includes('result') || s.heading.toLowerCase().includes('experiment')
          ).map((s: PaperSection) => s.id),
          include_abstract: false,
          include_unassigned: false,
        };
      default:
        return { include_sections: selectedSections, include_abstract: includeAbstract, include_unassigned: includeUnassigned };
    }
  };

  const handleParsePaper = async () => {
    if (!filePath) return;
    
    setParsingPaper(true);
    setError(null);
    setParsedPaper(null);
    
    try {
      const paper = await api.parsePdfAsResearchPaper(filePath);
      setParsedPaper(paper);
      // Select all sections by default
      setSelectedSections(paper.sections.map((s: PaperSection) => s.id));
      setPaperPreset('full');
    } catch (err: any) {
      setError(`Failed to parse paper: ${err.toString()}`);
    } finally {
      setParsingPaper(false);
    }
  };

  const toggleSection = (sectionId: string) => {
    setPaperPreset('custom');
    setSelectedSections(prev =>
      prev.includes(sectionId)
        ? prev.filter(id => id !== sectionId)
        : [...prev, sectionId]
    );
  };

  const getTotalTokens = () => {
    if (!parsedPaper) return 0;
    let total = 0;
    if (includeAbstract && parsedPaper.abstract_text) {
      total += Math.ceil(parsedPaper.abstract_text.length / 4);
    }
    for (const section of parsedPaper.sections) {
      if (selectedSections.includes(section.id)) {
        total += section.token_estimate;
      }
    }
    if (includeUnassigned && parsedPaper.unassigned_content) {
      total += Math.ceil(parsedPaper.unassigned_content.length / 4);
    }
    return total;
  };

  if (!isOpen) return null;

  const handleFileSelect = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: 'Documents & code',
            extensions: [
              'json', 'jsonl', 'csv', 'txt', 'md', 'pdf', 'doc', 'docx', 'rtf', 'odt',
              'py', 'js', 'ts', 'tsx', 'jsx', 'rs', 'go', 'java', 'kt', 'swift',
              'c', 'cpp', 'h', 'hpp', 'cs', 'rb', 'php', 'sh', 'bash', 'sql',
              'yaml', 'yml', 'toml', 'ini', 'xml', 'html', 'css', 'scss', 'vue', 'svelte',
            ],
          },
          { name: 'All files', extensions: ['*'] },
        ],
      });
      
      if (selected && typeof selected === 'string') {
        setFilePath(selected);
        setFolderPath('');
        // Auto-detect format from extension
        const ext = selected.toLowerCase().split('.').pop();
        if (ext === 'jsonl') setFormat('jsonl');
        else if (ext === 'csv') setFormat('csv');
        else if (ext === 'txt' || ext === 'md') setFormat('txt');
        else if (ext === 'json') setFormat('json');
        else setFormat('auto');
      }
    } catch (error) {
      console.error('File selection cancelled or failed:', error);
    }
  };

  const handleFolderSelect = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
      });
      
      if (selected) {
        // Handle both string and string[] return types
        const path = Array.isArray(selected) ? selected[0] : selected;
        if (path && typeof path === 'string') {
          setFolderPath(path);
        }
      } else {
        // User cancelled - this is normal, no error
        console.log('Folder selection cancelled');
      }
    } catch (error) {
      console.error('Folder selection failed:', error);
      setError(`Failed to open folder dialog: ${error instanceof Error ? error.message : String(error)}`);
    }
  };

  const runImportLogic = async (onProgress?: (msg: string) => void): Promise<void> => {
    setLoading(true);
    setError(null);
    setResult(null);
    const report = (msg: string) => { onProgress?.(msg); };

    try {
      let importResult;
      
      if (importType === 'file') {
        if (!filePath && !folderPath && !textContent) {
          setError('Please select a file, folder, or paste content');
          setLoading(false);
          return;
        }
        
        // If folder selected in file mode, use folder import
        if (folderPath) {
          report(`Importing from folder: ${folderPath.split(/[/\\]/).pop() || folderPath}...`);
          importResult = await api.importTrainingDataFromFolder({
            project_id: projectId,
            local_model_id: localModelId,
            folder_path: folderPath,
            include_subfolders: includeSubfolders,
          });
        } else if (textContent) {
        // If we have text content from file reader, use text import
          report('Importing from pasted text...');
          importResult = await api.importTrainingDataFromText(
            projectId,
            localModelId || null,
            textContent,
            format === 'auto' ? 'txt' : format
          );
        } else {
          // Use file path with Tauri
          report(`Importing file: ${filePath.split(/[/\\]/).pop()}...`);
          importResult = await api.importTrainingDataFromFile({
            project_id: projectId,
            local_model_id: localModelId,
            source_type: 'local_file',
            source_path: filePath,
            format: format === 'auto' ? 'auto' : format,
          });
        }
      } else if (importType === 'folder') {
        if (!folderPath) {
          setError('Please select a folder');
          setLoading(false);
          return;
        }
        
        report(`Importing from folder: ${folderPath.split(/[/\\]/).pop() || folderPath}...`);
        importResult = await api.importTrainingDataFromFolder({
          project_id: projectId,
          local_model_id: localModelId,
          folder_path: folderPath,
          include_subfolders: includeSubfolders,
        });
      } else if (importType === 'url') {
        if (!url.trim()) {
          setError('Please enter a URL');
          setLoading(false);
          return;
        }
        
        report(`Fetching from URL: ${url.slice(0, 50)}${url.length > 50 ? '...' : ''}`);
        importResult = await api.importTrainingDataFromUrl({
          project_id: projectId,
          local_model_id: localModelId,
          source_type: 'url',
          source_path: url,
          format,
        });
      } else if (importType === 'coder_history') {
        const workspacePathToUse = folderPath || workspacePath;
        if (!workspacePathToUse.trim()) {
          setError('Please select a workspace folder or ensure Simple Coder has a workspace open');
          setLoading(false);
          return;
        }
        
        report(`Importing from coder history: ${workspacePathToUse.split(/[/\\]/).pop() || 'workspace'}...`);
        importResult = await api.importTrainingDataFromCoderHistory({
          project_id: projectId,
          local_model_id: localModelId || undefined,
          workspace_path: workspacePathToUse,
        });
      } else if (importType === 'profile_chat') {
        report(`Importing from profile chat${selectedProfileId ? ` (profile selected)` : ''}...`);
        importResult = await api.importTrainingDataFromChatMessages({
          project_id: projectId,
          local_model_id: localModelId || undefined,
          profile_id: selectedProfileId || undefined,
        });
      } else if (importType === 'research_paper') {
        const pdfPaths = paperPdfPaths.length > 0 ? paperPdfPaths : (filePath ? [filePath] : []);
        if (pdfPaths.length === 0) {
          setError('Please select a PDF file or folder');
          setLoading(false);
          return;
        }

        let totalSuccess = 0;
        let totalError = 0;
        const allErrors: string[] = [];
        const total = pdfPaths.length;

        for (let i = 0; i < pdfPaths.length; i++) {
          const pdfPath = pdfPaths[i];
          const name = pdfPath.split(/[/\\]/).pop() || 'paper.pdf';
          report(total > 1 ? `Parsing ${i + 1}/${total}: ${name}...` : `Parsing ${name}...`);
          let paperToUse = pdfPath === filePath ? parsedPaper : null;
          let opts: { include_sections: string[]; include_abstract: boolean; include_unassigned: boolean };
          if (!paperToUse) {
            try {
              paperToUse = await api.parsePdfAsResearchPaper(pdfPath);
            } catch (parseErr: any) {
              allErrors.push(`${pdfPath.split(/[/\\]/).pop()}: ${parseErr?.message || 'Parse failed'}`);
              totalError += 1;
              continue;
            }
          }
          opts = getImportOptionsFromPreset(paperToUse);

          try {
            const res = await api.importResearchPaper({
              project_id: projectId,
              local_model_id: localModelId,
              file_path: pdfPath,
              include_sections: opts.include_sections,
              include_abstract: opts.include_abstract,
              include_unassigned: opts.include_unassigned,
              chunk_by_section: chunkBySection,
            });
            totalSuccess += res.success_count;
            totalError += res.error_count;
            if (res.errors?.length) allErrors.push(...res.errors);
          } catch (e: any) {
            allErrors.push(`${pdfPath.split(/[/\\]/).pop()}: ${e?.message || 'Import failed'}`);
            totalError += 1;
          }
        }

        importResult = {
          success_count: totalSuccess,
          error_count: totalError,
          errors: allErrors,
        };
      } else {
        // text
        if (!textContent.trim()) {
          setError('Please enter or paste training data');
          setLoading(false);
          return;
        }
        
        report('Importing from pasted text...');
        importResult = await api.importTrainingDataFromText(
          projectId,
          localModelId || null,
          textContent,
          format
        );
      }

      setResult(importResult);
      if (importResult.success_count > 0) {
        onImportComplete();
        setTimeout(() => onClose(), 500);
      }
    } catch (err: any) {
      setError(err.toString());
    } finally {
      setLoading(false);
    }
  };

  const handleImport = async () => {
    if (onImportRequest) {
      // Parent runs import in background, shows status on main screen
      onImportRequest(runImportLogic);
      onClose();
    } else {
      await runImportLogic();
    }
  };

  return (
    <div
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        background: 'rgba(0, 0, 0, 0.5)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1000,
      }}
      onClick={() => { /* Persist: do not close on overlay click - user must use X or Cancel */ }}
    >
      <div
        className="card"
        style={{
          width: '90%',
          maxWidth: '600px',
          maxHeight: '90vh',
          overflow: 'auto',
          background: 'var(--card-bg)',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '20px' }}>
          <h2 style={{ margin: 0 }}>📥 Import Training Data</h2>
          <button
            onClick={onClose}
            style={{
              background: 'transparent',
              border: 'none',
              fontSize: '24px',
              cursor: 'pointer',
              color: 'var(--text-primary)',
            }}
          >
            ×
          </button>
        </div>

        {/* Import Type Selection */}
        <div style={{ marginBottom: '20px' }}>
          <label style={{ display: 'block', marginBottom: '8px', fontWeight: '500' }}>
            Import Source
          </label>
          <div style={{ display: 'flex', gap: '10px', flexWrap: 'wrap' }}>
            <button
              className={`btn ${importType === 'file' ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setImportType('file')}
            >
              📄 Single File
            </button>
            <button
              className={`btn ${importType === 'folder' ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setImportType('folder')}
            >
              📁 Folder
            </button>
            <button
              className={`btn ${importType === 'url' ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setImportType('url')}
            >
              🌐 URL/Server
            </button>
            <button
              className={`btn ${importType === 'text' ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setImportType('text')}
            >
              📝 Paste Text
            </button>
            <button
              className={`btn ${importType === 'coder_history' ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setImportType('coder_history')}
            >
              💬 Simple Coder History
            </button>
            <button
              className={`btn ${importType === 'profile_chat' ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setImportType('profile_chat')}
            >
              💬 Profile Chat
            </button>
            <button
              className={`btn ${importType === 'research_paper' ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setImportType('research_paper')}
            >
              📄 Research Paper
            </button>
          </div>
        </div>

        {/* Format Selection - hide for coder_history and profile_chat */}
        {importType !== 'coder_history' && importType !== 'profile_chat' && (
        <div style={{ marginBottom: '20px' }}>
          <label style={{ display: 'block', marginBottom: '8px', fontWeight: '500' }}>
            File Format
          </label>
          <select
            value={format}
            onChange={(e) => setFormat(e.target.value)}
            style={{
              width: '100%',
              padding: '8px 12px',
              borderRadius: '4px',
              border: '1px solid var(--border-color)',
              background: 'var(--card-bg)',
              color: 'var(--text-primary)',
            }}
          >
            <option value="auto">Auto-detect (for folders/multiple files)</option>
            <option value="json">JSON (array of input/output objects)</option>
            <option value="jsonl">JSONL (one JSON object per line)</option>
            <option value="csv">CSV (input/output columns)</option>
            <option value="txt">Text (input/output pairs)</option>
          </select>
          <small style={{ color: 'var(--text-secondary)', display: 'block', marginTop: '5px' }}>
            Supported: JSON, JSONL, CSV, TXT, MD, PDF, DOC, DOCX, RTF, ODT, and code files (Py, JS, TS, Go, Rust, etc.)
          </small>
        </div>
        )}

        {/* Profile Chat - import from chat_messages */}
        {importType === 'profile_chat' && (
          <div style={{ marginBottom: '20px' }}>
            <label style={{ display: 'block', marginBottom: '8px', fontWeight: '500' }}>
              Profile (optional)
            </label>
            <select
              value={selectedProfileId}
              onChange={(e) => setSelectedProfileId(e.target.value)}
              style={{
                width: '100%',
                padding: '8px 12px',
                borderRadius: '4px',
                border: '1px solid var(--border-color)',
                background: 'var(--card-bg)',
                color: 'var(--text-primary)',
              }}
            >
              <option value="">All profiles (all chat conversations)</option>
              {profiles.map((p) => (
                <option key={p.id} value={p.id}>{p.name || p.id}</option>
              ))}
            </select>
            <small style={{ color: 'var(--text-secondary)', display: 'block', marginTop: '5px' }}>
              Imports user/assistant message pairs from Profile Chat. Select a profile to import only that conversation, or leave as &quot;All profiles&quot; to import everything.
            </small>
          </div>
        )}

        {/* Coder History - workspace with panther_chat_history */}
        {importType === 'coder_history' && (
          <div style={{ marginBottom: '20px' }}>
            <label style={{ display: 'block', marginBottom: '8px', fontWeight: '500' }}>
              Workspace (panther_chat_history folder)
            </label>
            <div style={{ display: 'flex', gap: '10px', alignItems: 'center' }}>
              <input
                type="text"
                value={folderPath || workspacePath}
                placeholder="Workspace path (auto-detected) or select folder..."
                readOnly
                style={{
                  flex: 1,
                  padding: '8px 12px',
                  borderRadius: '4px',
                  border: '1px solid var(--border-color)',
                  background: 'var(--bg-secondary)',
                  color: 'var(--text-primary)',
                }}
              />
              <button className="btn btn-secondary" onClick={async () => {
                try {
                  const selected = await open({ directory: true, multiple: false });
                  if (selected) {
                    const path = Array.isArray(selected) ? selected[0] : selected;
                    if (path && typeof path === 'string') setFolderPath(path);
                  }
                } catch (e) {
                  console.error('Folder selection failed:', e);
                }
              }}>
                📂 Select Folder
              </button>
            </div>
            <small style={{ color: 'var(--text-secondary)', display: 'block', marginTop: '5px' }}>
              Imports Q&A pairs from panther_chat_history/*.md in the workspace. Use Simple Coder first to create conversations.
            </small>
          </div>
        )}

        {/* Research Paper Import */}
        {importType === 'research_paper' && (
          <div style={{ marginBottom: '20px' }}>
            <label style={{ display: 'block', marginBottom: '8px', fontWeight: '500' }}>
              Select PDF File or Folder
            </label>
            <div style={{ display: 'flex', gap: '10px', flexWrap: 'wrap' }}>
              <input
                type="text"
                value={filePath || paperFolderPath}
                placeholder="Select a PDF file or folder of PDFs..."
                readOnly
                style={{
                  flex: 1,
                  minWidth: '200px',
                  padding: '8px 12px',
                  borderRadius: '4px',
                  border: '1px solid var(--border-color)',
                  background: 'var(--bg-secondary)',
                  color: 'var(--text-primary)',
                }}
              />
              <button className="btn btn-secondary" onClick={async () => {
                try {
                  const selected = await open({
                    multiple: false,
                    filters: [{ name: 'PDF', extensions: ['pdf'] }],
                  });
                  if (selected && typeof selected === 'string') {
                    setFilePath(selected);
                    setPaperFolderPath('');
                    setPaperPdfPaths([]);
                    setParsedPaper(null);
                  }
                } catch (e) {
                  console.error('File selection failed:', e);
                }
              }}>
                📄 File
              </button>
              <button className="btn btn-secondary" onClick={async () => {
                try {
                  const selected = await open({ directory: true, multiple: false });
                  const path = selected ? (Array.isArray(selected) ? selected[0] : selected) : null;
                  if (path && typeof path === 'string') {
                    setPaperFolderPath(path);
                    setFilePath('');
                    setParsedPaper(null);
                    setLoading(true);
                    try {
                      const pdfs = await api.listPdfFilesInFolder(path, paperFolderRecursive);
                      setPaperPdfPaths(pdfs);
                    } catch (e) {
                      console.error('Failed to list PDFs:', e);
                      setPaperPdfPaths([]);
                    } finally {
                      setLoading(false);
                    }
                  }
                } catch (e) {
                  console.error('Folder selection failed:', e);
                }
              }}>
                📁 Folder
              </button>
              {paperPdfPaths.length > 0 && (
                <span style={{ fontSize: '12px', color: 'var(--text-secondary)', alignSelf: 'center' }}>
                  {paperPdfPaths.length} PDF{paperPdfPaths.length !== 1 ? 's' : ''} found
                </span>
              )}
              {filePath && (
                <button 
                  className="btn btn-primary" 
                  onClick={handleParsePaper}
                  disabled={parsingPaper}
                >
                  {parsingPaper ? '⏳ Parsing...' : '🔍 Parse'}
                </button>
              )}
            </div>
            {paperFolderPath && paperPdfPaths.length > 0 && (
              <>
                <label style={{ display: 'flex', alignItems: 'center', gap: '8px', marginTop: '10px', cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={paperFolderRecursive}
                    onChange={(e) => {
                      setPaperFolderRecursive(e.target.checked);
                      api.listPdfFilesInFolder(paperFolderPath, e.target.checked).then(setPaperPdfPaths).catch(() => setPaperPdfPaths([]));
                    }}
                  />
                  <span style={{ fontSize: '12px' }}>Include subfolders</span>
                </label>
                <div style={{ marginTop: '10px' }}>
                  <span style={{ fontSize: '12px', fontWeight: 500, marginRight: '8px' }}>Preset for all:</span>
                  <div style={{ display: 'flex', gap: '6px', flexWrap: 'wrap', marginTop: '4px' }}>
                    {(['full', 'abstract_conclusions', 'methods_results'] as const).map((p) => (
                      <button
                        key={p}
                        className={`btn btn-sm ${paperPreset === p ? 'btn-primary' : 'btn-secondary'}`}
                        onClick={() => setPaperPreset(p)}
                        style={{ padding: '4px 10px', fontSize: '11px' }}
                      >
                        {p === 'full' ? 'Full Paper' : p === 'abstract_conclusions' ? 'Abstract + Conclusions' : 'Methods + Results'}
                      </button>
                    ))}
                  </div>
                </div>
              </>
            )}
            
            {/* Parsed Paper Preview */}
            {parsedPaper && (
              <div style={{ marginTop: '15px', border: '1px solid var(--border-color)', borderRadius: '8px', padding: '15px' }}>
                {/* Paper Info */}
                <div style={{ marginBottom: '15px' }}>
                  {parsedPaper.title && (
                    <h4 style={{ margin: '0 0 8px 0', fontSize: '14px' }}>{parsedPaper.title}</h4>
                  )}
                  <div style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
                    {parsedPaper.metadata.year && <span>Year: {parsedPaper.metadata.year} | </span>}
                    {parsedPaper.metadata.doi && <span>DOI: {parsedPaper.metadata.doi} | </span>}
                    {parsedPaper.metadata.arxiv_id && <span>arXiv: {parsedPaper.metadata.arxiv_id}</span>}
                  </div>
                  {parsedPaper.parsing_warnings.length > 0 && (
                    <div style={{ marginTop: '8px', padding: '8px', background: 'rgba(255, 193, 7, 0.1)', borderRadius: '4px', fontSize: '11px', color: '#ffc107' }}>
                      ⚠️ {parsedPaper.parsing_warnings.join(' | ')}
                    </div>
                  )}
                </div>

                {/* Presets */}
                <div style={{ marginBottom: '15px' }}>
                  <label style={{ display: 'block', marginBottom: '8px', fontWeight: '500', fontSize: '13px' }}>
                    Section Preset
                  </label>
                  <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                    <button 
                      className={`btn btn-sm ${paperPreset === 'full' ? 'btn-primary' : 'btn-secondary'}`}
                      onClick={() => setPaperPreset('full')}
                      style={{ padding: '4px 10px', fontSize: '12px' }}
                    >
                      Full Paper
                    </button>
                    <button 
                      className={`btn btn-sm ${paperPreset === 'abstract_conclusions' ? 'btn-primary' : 'btn-secondary'}`}
                      onClick={() => setPaperPreset('abstract_conclusions')}
                      style={{ padding: '4px 10px', fontSize: '12px' }}
                    >
                      Abstract + Conclusions
                    </button>
                    <button 
                      className={`btn btn-sm ${paperPreset === 'methods_results' ? 'btn-primary' : 'btn-secondary'}`}
                      onClick={() => setPaperPreset('methods_results')}
                      style={{ padding: '4px 10px', fontSize: '12px' }}
                    >
                      Methods + Results
                    </button>
                    <button 
                      className={`btn btn-sm ${paperPreset === 'custom' ? 'btn-primary' : 'btn-secondary'}`}
                      onClick={() => setPaperPreset('custom')}
                      style={{ padding: '4px 10px', fontSize: '12px' }}
                    >
                      Custom
                    </button>
                  </div>
                </div>

                {/* Section Selection */}
                <div style={{ marginBottom: '15px' }}>
                  <label style={{ display: 'block', marginBottom: '8px', fontWeight: '500', fontSize: '13px' }}>
                    Sections to Include
                  </label>
                  <div style={{ maxHeight: '200px', overflow: 'auto', border: '1px solid var(--border-color)', borderRadius: '4px', padding: '8px' }}>
                    {/* Abstract */}
                    {parsedPaper.abstract_text && (
                      <label style={{ display: 'flex', alignItems: 'center', gap: '8px', padding: '4px 0', cursor: 'pointer' }}>
                        <input
                          type="checkbox"
                          checked={includeAbstract}
                          onChange={(e) => {
                            setIncludeAbstract(e.target.checked);
                            setPaperPreset('custom');
                          }}
                        />
                        <span style={{ flex: 1, fontSize: '12px' }}>Abstract</span>
                        <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
                          ~{Math.ceil(parsedPaper.abstract_text.length / 4)} tokens
                        </span>
                      </label>
                    )}
                    
                    {/* Sections */}
                    {parsedPaper.sections.map((section) => (
                      <label 
                        key={section.id} 
                        style={{ 
                          display: 'flex', 
                          alignItems: 'center', 
                          gap: '8px', 
                          padding: '4px 0', 
                          paddingLeft: section.level > 1 ? '20px' : '0',
                          cursor: 'pointer' 
                        }}
                      >
                        <input
                          type="checkbox"
                          checked={selectedSections.includes(section.id)}
                          onChange={() => toggleSection(section.id)}
                        />
                        <span style={{ flex: 1, fontSize: '12px' }}>{section.heading}</span>
                        <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
                          ~{section.token_estimate} tokens
                        </span>
                      </label>
                    ))}
                    
                    {/* Unassigned content */}
                    {parsedPaper.unassigned_content && parsedPaper.unassigned_content.length > 100 && (
                      <label style={{ display: 'flex', alignItems: 'center', gap: '8px', padding: '4px 0', cursor: 'pointer', opacity: 0.7 }}>
                        <input
                          type="checkbox"
                          checked={includeUnassigned}
                          onChange={(e) => {
                            setIncludeUnassigned(e.target.checked);
                            setPaperPreset('custom');
                          }}
                        />
                        <span style={{ flex: 1, fontSize: '12px' }}>Other content (unstructured)</span>
                        <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
                          ~{Math.ceil(parsedPaper.unassigned_content.length / 4)} tokens
                        </span>
                      </label>
                    )}
                  </div>
                </div>

                {/* Token Summary */}
                <div style={{ 
                  padding: '10px', 
                  background: 'var(--bg-secondary)', 
                  borderRadius: '4px', 
                  marginBottom: '15px',
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center'
                }}>
                  <span style={{ fontSize: '13px', fontWeight: '500' }}>
                    Total: ~{getTotalTokens().toLocaleString()} tokens
                  </span>
                  {getTotalTokens() > 8000 && (
                    <span style={{ fontSize: '11px', color: '#ffc107' }}>
                      ⚠️ Large - consider selecting fewer sections
                    </span>
                  )}
                </div>

                {/* Import Options */}
                <div style={{ marginBottom: '10px' }}>
                  <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
                    <input
                      type="checkbox"
                      checked={chunkBySection}
                      onChange={(e) => setChunkBySection(e.target.checked)}
                    />
                    <span style={{ fontSize: '12px' }}>Create separate training examples per section</span>
                  </label>
                  <small style={{ color: 'var(--text-secondary)', display: 'block', marginLeft: '24px', fontSize: '11px' }}>
                    {chunkBySection 
                      ? 'Each section becomes a separate training example (recommended for fine-tuning)'
                      : 'All selected sections combined into one training example'}
                  </small>
                </div>
              </div>
            )}
            
            {paperPdfPaths.length > 0 && (
              <small style={{ color: 'var(--text-secondary)', display: 'block', marginTop: '8px' }}>
                Folder import uses the selected preset for all {paperPdfPaths.length} PDFs. Parse a single file to customize sections.
              </small>
            )}
            <small style={{ color: 'var(--text-secondary)', display: 'block', marginTop: '8px' }}>
              Parse academic papers to extract sections, abstract, and metadata. Section detection is best-effort heuristic.
            </small>
          </div>
        )}

        {/* File Input */}
        {importType === 'file' && (
          <div style={{ marginBottom: '20px' }}>
            <label style={{ display: 'block', marginBottom: '8px', fontWeight: '500' }}>
              Select File or Folder
            </label>
            <div style={{ display: 'flex', gap: '10px', flexWrap: 'wrap' }}>
              <input
                type="text"
                value={filePath || folderPath}
                placeholder="Select a file or folder..."
                readOnly
                style={{
                  flex: 1,
                  minWidth: '200px',
                  padding: '8px 12px',
                  borderRadius: '4px',
                  border: '1px solid var(--border-color)',
                  background: 'var(--bg-secondary)',
                  color: 'var(--text-primary)',
                }}
              />
              <button className="btn btn-secondary" onClick={handleFileSelect}>
                📄 File
              </button>
              <button className="btn btn-secondary" onClick={async () => {
                try {
                  const selected = await open({ directory: true, multiple: false });
                  if (selected) {
                    const path = Array.isArray(selected) ? selected[0] : selected;
                    if (path && typeof path === 'string') {
                      setFolderPath(path);
                      setFilePath('');
                    }
                  }
                } catch (e) {
                  console.error('Folder selection failed:', e);
                }
              }}>
                📁 Folder
              </button>
            </div>
            {(filePath || folderPath) && (
              <div style={{ marginTop: '10px', padding: '10px', background: 'var(--bg-secondary)', borderRadius: '4px', fontSize: '12px', color: 'var(--text-secondary)' }}>
                Selected: {(filePath || folderPath).split(/[/\\]/).pop()}
              </div>
            )}
          </div>
        )}

        {/* Folder Input */}
        {importType === 'folder' && (
          <div style={{ marginBottom: '20px' }}>
            <label style={{ display: 'block', marginBottom: '8px', fontWeight: '500' }}>
              Select Folder
            </label>
            <div style={{ display: 'flex', gap: '10px', alignItems: 'center' }}>
              <input
                type="text"
                value={folderPath}
                placeholder="Folder path or click to browse..."
                readOnly
                style={{
                  flex: 1,
                  padding: '8px 12px',
                  borderRadius: '4px',
                  border: '1px solid var(--border-color)',
                  background: 'var(--bg-secondary)',
                  color: 'var(--text-primary)',
                }}
              />
              <button className="btn btn-secondary" onClick={handleFolderSelect}>
                📂 Browse Folder
              </button>
            </div>
            {folderPath && (
              <div style={{ marginTop: '10px' }}>
                <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={includeSubfolders}
                    onChange={(e) => setIncludeSubfolders(e.target.checked)}
                  />
                  <span style={{ fontSize: '13px' }}>Include subfolders (recursive scan)</span>
                </label>
                <div style={{ marginTop: '8px', padding: '10px', background: 'var(--bg-secondary)', borderRadius: '4px', fontSize: '12px', color: 'var(--text-secondary)' }}>
                  Selected: {folderPath}
                  <br />
                  {includeSubfolders ? 'Will scan all files in folder and subfolders' : 'Will scan files in folder only'}
                </div>
              </div>
            )}
          </div>
        )}

        {/* URL Input */}
        {importType === 'url' && (
          <div style={{ marginBottom: '20px' }}>
            <label style={{ display: 'block', marginBottom: '8px', fontWeight: '500' }}>
              URL or Server Endpoint
            </label>
            <input
              type="text"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://example.com/training-data.json"
              style={{
                width: '100%',
                padding: '8px 12px',
                borderRadius: '4px',
                border: '1px solid var(--border-color)',
                background: 'var(--card-bg)',
                color: 'var(--text-primary)',
              }}
            />
            <small style={{ color: 'var(--text-secondary)', display: 'block', marginTop: '5px' }}>
              Supports HTTP/HTTPS URLs. The server should return data in the selected format.
            </small>
          </div>
        )}

        {/* Text Input */}
        {(importType === 'text' || (importType === 'file' && textContent)) && (
          <div style={{ marginBottom: '20px' }}>
            <label style={{ display: 'block', marginBottom: '8px', fontWeight: '500' }}>
              Training Data Content
            </label>
            <textarea
              value={textContent}
              onChange={(e) => setTextContent(e.target.value)}
              placeholder={
                format === 'json'
                  ? '[\n  {"input": "question?", "output": "answer"},\n  ...\n]'
                  : format === 'jsonl'
                  ? '{"input": "question?", "output": "answer"}\n{"input": "question2?", "output": "answer2"}\n...'
                  : format === 'csv'
                  ? 'input,output\n"question?", "answer"\n"question2?", "answer2"\n...'
                  : 'Input text here\n\nOutput text here\n\nInput 2\n\nOutput 2\n...'
              }
              style={{
                width: '100%',
                minHeight: '200px',
                padding: '10px',
                borderRadius: '4px',
                border: '1px solid var(--border-color)',
                background: 'var(--card-bg)',
                color: 'var(--text-primary)',
                fontFamily: 'monospace',
                fontSize: '13px',
              }}
            />
          </div>
        )}

        {/* Error Display */}
        {error && (
          <div style={{
            padding: '10px',
            background: 'rgba(220, 53, 69, 0.1)',
            color: '#dc3545',
            borderRadius: '4px',
            marginBottom: '15px',
          }}>
            ❌ {error}
          </div>
        )}

        {/* Result Display */}
        {result && (
          <div style={{
            padding: '15px',
            background: result.error_count > 0 ? 'rgba(255, 193, 7, 0.1)' : 'rgba(40, 167, 69, 0.1)',
            borderRadius: '4px',
            marginBottom: '15px',
          }}>
            <div style={{ fontWeight: 'bold', marginBottom: '10px' }}>
              {result.success_count > 0 ? '✅' : '⚠️'} Import Results
            </div>
            <div>✅ Successfully imported: {result.success_count} items</div>
            {result.error_count > 0 && (
              <div style={{ color: '#ffc107', marginTop: '5px' }}>
                ⚠️ Failed: {result.error_count} items
              </div>
            )}
            {result.errors && result.errors.length > 0 && (
              <div style={{ marginTop: '10px', fontSize: '12px' }}>
                <strong>Errors:</strong>
                <ul style={{ margin: '5px 0 0 20px' }}>
                  {result.errors.map((err: string, idx: number) => (
                    <li key={idx}>{err}</li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        )}

        {/* Actions */}
        <div style={{ display: 'flex', gap: '10px', justifyContent: 'flex-end' }}>
          <button className="btn btn-secondary" onClick={onClose} disabled={loading}>
            Cancel
          </button>
          <button
            className="btn btn-primary"
            onClick={handleImport}
            disabled={loading || (importType === 'file' && !filePath && !folderPath && !textContent) || (importType === 'folder' && !folderPath) || (importType === 'url' && !url) || (importType === 'text' && !textContent) || (importType === 'coder_history' && !folderPath && !workspacePath) || (importType === 'research_paper' && !filePath && paperPdfPaths.length === 0)}
          >
            {loading ? '⏳ Importing...' : '📥 Import'}
          </button>
        </div>
      </div>
    </div>
  );
}

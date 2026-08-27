import React from 'react'
import NodeViewer from './NodeViewer'
import GroupViewer from './GroupViewer'
import EntryForm from './EntryForm'
import NavBar from './NavBar'
import TreeViewer from './TreeViewer'
import withNavigateHook from './nagivateHook'
import { IconSave, IconX } from './Icons'

class Viewport extends React.Component {
    constructor(props) {
        super(props)
        this.onGroupSelect  = this.onGroupSelect.bind(this)
        this.onSelect       = this.onSelect.bind(this)
        this.onSearch       = this.onSearch.bind(this)
        this.onGroupRenamed = this.onGroupRenamed.bind(this)
        this.onGroupCreated = this.onGroupCreated.bind(this)
        this.onNewEntry     = this.onNewEntry.bind(this)
        this.onEditEntry    = this.onEditEntry.bind(this)
        this.onFormSaved    = this.onFormSaved.bind(this)
        this.onFormCancel   = this.onFormCancel.bind(this)
        this.onEntryDeleted = this.onEntryDeleted.bind(this)
        this.onAddRootGroup = this.onAddRootGroup.bind(this)
        this.onSaveDb       = this.onSaveDb.bind(this)
        this.toggleSidebar  = this.toggleSidebar.bind(this)

        this.state = {
            tree:         {},
            group:        null,
            groupDepth:   0,
            groupMask:    false,
            rightPanel:   { mode: 'none', entry: null },
            nodeMask:     false,
            sidebarOpen:  false,
            // save-db modal
            saveModal:    false,
            savePassword: '',
            saveStatus:   '',
        }
    }

    // ── Tree helpers ──────────────────────────────────────────────

    findGroupDepth(group, targetId, depth = 0) {
        if (group.id === targetId) return depth
        for (const child of (group.children || [])) {
            const d = this.findGroupDepth(child, targetId, depth + 1)
            if (d >= 0) return d
        }
        return -1
    }

    // ── Group selection ───────────────────────────────────────────

    onGroupSelect(group) {
        if (!group || !group.id) return
        if (this.state.group && this.state.group.id === group.id) return

        if (this.serverRequest) this.serverRequest.abort()

        const depth = this.findGroupDepth(this.state.tree, group.id)
        this.setState({
            groupMask:  true,
            groupDepth: depth >= 0 ? depth : 0,
            rightPanel: { mode: 'none', entry: null },
            sidebarOpen: false,
        })

        this.serverRequest = KeePass4Web.fetch('get_group_entries', {
            method: 'GET',
            data: { id: group.id },
            success: (data) => this.setState({ group: data }),
            error: KeePass4Web.error.bind(this),
            complete: () => this.setState({ groupMask: false }),
        })
    }

    // ── Entry selection (view detail) ─────────────────────────────

    onSelect(entry) {
        if (!entry || !entry.id) return
        if (this.serverRequest) this.serverRequest.abort()

        this.setState({ nodeMask: true, rightPanel: { mode: 'view', entry: null } })

        this.serverRequest = KeePass4Web.fetch('get_entry', {
            method: 'GET',
            data: { id: entry.id },
            success: (data) => this.setState({ rightPanel: { mode: 'view', entry: data } }),
            error: KeePass4Web.error.bind(this),
            complete: () => this.setState({ nodeMask: false }),
        })
    }

    // ── Search ───────────────────────────────────────────────────

    onSearch(refs, event) {
        event.preventDefault()
        if (this.serverRequest) this.serverRequest.abort()
        this.setState({ groupMask: true, rightPanel: { mode: 'none', entry: null } })
        this.serverRequest = KeePass4Web.fetch('search_entries', {
            method: 'GET',
            data: { term: refs.term.value.replace(/^\s+|\s+$/g, '') },
            success: (data) => this.setState({ group: data }),
            error: KeePass4Web.error.bind(this),
            complete: () => this.setState({ groupMask: false }),
        })
    }

    // ── Group mutations ───────────────────────────────────────────

    onGroupCreated() {
        KeePass4Web.fetch('get_groups', {
            method: 'GET',
            success: (data) => this.setState({ tree: data.groups }),
            error: KeePass4Web.error.bind(this),
        })
    }

    onGroupRenamed(groupId, newTitle) {
        const patchTree = (groups) => groups.map(g => ({
            ...g,
            title:    g.id === groupId ? newTitle : g.title,
            children: g.children ? patchTree(g.children) : g.children,
        }))
        this.setState(prev => ({
            tree:  { ...prev.tree, children: patchTree(prev.tree.children || []) },
            group: prev.group && prev.group.id === groupId
                ? { ...prev.group, title: newTitle }
                : prev.group,
        }))
    }

    onAddRootGroup() {
        const name = window.prompt('New group name:')
        if (!name || !name.trim()) return
        KeePass4Web.fetch('group', {
            method: 'POST',
            data: { parent_id: this.state.tree.id, title: name.trim() },
            success: () => this.onGroupCreated(),
            error: KeePass4Web.error.bind(this),
        })
    }

    // ── Entry mutations ───────────────────────────────────────────

    refreshGroup() {
        if (!this.state.group || !this.state.group.id) return
        KeePass4Web.fetch('get_group_entries', {
            method: 'GET',
            data: { id: this.state.group.id },
            success: (data) => this.setState({ group: data }),
            error: KeePass4Web.error.bind(this),
        })
    }

    onNewEntry() {
        this.setState({ rightPanel: { mode: 'new', entry: null } })
    }

    onEditEntry(entry) {
        if (!entry || !entry.id) return
        if (this.serverRequest) this.serverRequest.abort()
        this.setState({ nodeMask: true })
        this.serverRequest = KeePass4Web.fetch('get_entry', {
            method: 'GET',
            data: { id: entry.id },
            success: (data) => this.setState({ rightPanel: { mode: 'edit', entry: data } }),
            error: KeePass4Web.error.bind(this),
            complete: () => this.setState({ nodeMask: false }),
        })
    }

    onFormSaved(savedId) {
        this.refreshGroup()
        // After saving, switch to view mode for the saved entry
        if (savedId) {
            KeePass4Web.fetch('get_entry', {
                method: 'GET',
                data: { id: savedId },
                success: (data) => this.setState({ rightPanel: { mode: 'view', entry: data } }),
                error: () => this.setState({ rightPanel: { mode: 'none', entry: null } }),
            })
        } else {
            this.setState({ rightPanel: { mode: 'none', entry: null } })
        }
    }

    onFormCancel() {
        const { rightPanel } = this.state
        // If we were editing, go back to viewing
        if (rightPanel.mode === 'edit' && rightPanel.entry) {
            this.setState({ rightPanel: { mode: 'view', entry: rightPanel.entry } })
        } else {
            this.setState({ rightPanel: { mode: 'none', entry: null } })
        }
    }

    onEntryDeleted() {
        this.refreshGroup()
        this.setState({ rightPanel: { mode: 'none', entry: null } })
    }

    // ── Save DB ───────────────────────────────────────────────────

    onSaveDb(e) {
        e.preventDefault()
        this.setState({ saveStatus: 'Saving…' })
        KeePass4Web.fetch('save_db', {
            method: 'POST',
            data: { password: this.state.savePassword },
            success: () => this.setState({ saveModal: false, savePassword: '', saveStatus: '' }),
            error: (err) => this.setState({ saveStatus: (err && (err.msg || err.toString())) || 'Save failed' }),
        })
    }

    // ── Sidebar toggle (mobile) ───────────────────────────────────

    toggleSidebar() {
        this.setState(prev => ({ sidebarOpen: !prev.sidebarOpen }))
    }

    // ── Lifecycle ─────────────────────────────────────────────────

    componentDidMount() {
        KeePass4Web.fetch('get_groups', {
            method: 'GET',
            success: (data) => {
                // the selection reads the tree back out of the state, so it
                // waits until the state holds it
                this.setState({ tree: data.groups }, () => {
                    if (data.last_selected)
                        this.onGroupSelect({ id: data.last_selected })
                })
            },
            error: KeePass4Web.error.bind(this),
        })
    }

    componentWillUnmount() {
        if (this.serverRequest) this.serverRequest.abort()
    }

    render() {
        const { tree, group, groupDepth, groupMask, nodeMask, rightPanel, sidebarOpen } = this.state

        const detailOpen = rightPanel.mode !== 'none'

        const saveModal = this.state.saveModal ? (
            <div className="kp-modal-backdrop" onClick={() => this.setState({ saveModal: false, savePassword: '', saveStatus: '' })}>
                <div className="kp-modal" onClick={e => e.stopPropagation()}>
                    <div className="kp-modal-header">
                        <h3>Save Database</h3>
                        <button
                            className="kp-btn-icon"
                            onClick={() => this.setState({ saveModal: false, savePassword: '', saveStatus: '' })}
                        >
                            <IconX size={16}/>
                        </button>
                    </div>
                    <p style={{ fontSize: 13, color: 'var(--kp-text-muted)', marginBottom: 16 }}>
                        Re-enter the <strong>KeePass master password</strong> (not your login password) to write changes to the server file.
                    </p>
                    <form onSubmit={this.onSaveDb}>
                        <div className="kp-field" style={{ marginBottom: 12 }}>
                            <label>Master password</label>
                            <input
                                type="password"
                                className="kp-input"
                                placeholder="Master password"
                                autoFocus
                                value={this.state.savePassword}
                                onChange={e => this.setState({ savePassword: e.target.value, saveStatus: '' })}
                            />
                        </div>
                        {this.state.saveStatus && (
                            <p style={{ color: 'var(--kp-danger)', fontSize: 13, marginBottom: 8 }}>
                                {this.state.saveStatus}
                            </p>
                        )}
                        <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
                            <button type="button" className="kp-btn kp-btn-outline"
                                    onClick={() => this.setState({ saveModal: false, savePassword: '', saveStatus: '' })}>
                                Cancel
                            </button>
                            <button type="submit" className="kp-btn kp-btn-primary">
                                <IconSave size={13}/>
                                Save
                            </button>
                        </div>
                    </form>
                </div>
            </div>
        ) : null

        return (
            <div className="kp-app">
                <NavBar
                    showSearch
                    onSearch={this.onSearch}
                    onSaveDb={() => this.setState({ saveModal: true })}
                    onToggleSidebar={this.toggleSidebar}
                />

                {saveModal}

                <div className="kp-body">
                    {/* Mobile sidebar overlay */}
                    {sidebarOpen && (
                        <div className="kp-sidebar-overlay" onClick={() => this.setState({ sidebarOpen: false })}/>
                    )}

                    <TreeViewer
                        tree={tree}
                        nodeClick={this.onGroupSelect}
                        nodeIcon="48"
                        open={sidebarOpen}
                        onAddRootGroup={this.onAddRootGroup}
                    />

                    <GroupViewer
                        group={group}
                        groupDepth={groupDepth}
                        selectedEntryId={rightPanel.entry ? rightPanel.entry.id : null}
                        onSelect={this.onSelect}
                        mask={groupMask}
                        onNewEntry={this.onNewEntry}
                        onEditEntry={this.onEditEntry}
                        onEntryDeleted={this.onEntryDeleted}
                        onGroupRenamed={this.onGroupRenamed}
                        onGroupCreated={this.onGroupCreated}
                    />

                    <div className={`kp-detail${detailOpen ? ' open' : ''}`}>
                        {rightPanel.mode === 'view' && (
                            <NodeViewer
                                entry={rightPanel.entry}
                                timeoutSec={30000}
                                mask={nodeMask}
                                onEdit={() => rightPanel.entry && this.onEditEntry(rightPanel.entry)}
                            />
                        )}
                        {(rightPanel.mode === 'new' || rightPanel.mode === 'edit') && (
                            <EntryForm
                                mode={rightPanel.mode}
                                entry={rightPanel.entry}
                                group={group}
                                onSaved={this.onFormSaved}
                                onCancel={this.onFormCancel}
                            />
                        )}
                        {!detailOpen && (
                            <div className="kp-detail-empty">
                                <span>🔑</span>
                                Select an entry to view details
                            </div>
                        )}
                    </div>
                </div>
            </div>
        )
    }
}

export default withNavigateHook(Viewport)

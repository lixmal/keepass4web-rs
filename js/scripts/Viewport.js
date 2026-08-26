import React from 'react'
import NodeViewer from './NodeViewer'
import GroupViewer from './GroupViewer'
import NavBar from './NavBar'
import TreeViewer from './TreeViewer'
import withNavigateHook from './nagivateHook'


class Viewport extends React.Component {
    constructor(props) {
        super(props)
        this.onGroupSelect = this.onGroupSelect.bind(this)
        this.onSelect = this.onSelect.bind(this)
        this.onSearch = this.onSearch.bind(this)
        this.onEntryCreated = this.onEntryCreated.bind(this)
        this.onDeleteEntry = this.onDeleteEntry.bind(this)
        this.onSaveDb = this.onSaveDb.bind(this)
        this.onGroupRenamed = this.onGroupRenamed.bind(this)
        this.onGroupCreated = this.onGroupCreated.bind(this)

        this.state = {
            tree: {},
            entry: null,
            group: null,
            groupDepth: 0,
            saveModal: false,
            savePassword: '',
            saveStatus: '',
        }
    }

    findGroupDepth(group, targetId, depth = 0) {
        if (group.id === targetId) return depth
        for (const child of (group.children || [])) {
            const d = this.findGroupDepth(child, targetId, depth + 1)
            if (d >= 0) return d
        }
        return -1
    }

    scroll(id) {
        document.getElementById(id).scrollIntoView()
        if (window.scrollY)
            window.scroll(0, scrollY - 70)
    }

    onGroupSelect(group) {
        if (!group || !group.id) return
        if (this.state.group && this.state.group.id && group.id === this.state.group.id) return

        if (this.serverRequest)
            this.serverRequest.abort()

        const depth = this.findGroupDepth(this.state.tree, group.id)
        this.setState({ entry: null, groupMask: true, groupDepth: depth >= 0 ? depth : 0 })

        this.serverRequest = KeePass4Web.fetch('get_group_entries', {
            method: 'GET',
            data: { id: group.id },
            success: (data) => {
                this.setState({ group: data })
                this.scroll('group-viewer')
            },
            error: KeePass4Web.error.bind(this),
            complete: () => this.setState({ groupMask: false }),
        })
    }

    onSelect(entry) {
        if (!entry || !entry.id) return
        if (this.state.entry && this.state.entry.id && entry.id === this.state.entry.id) return

        if (this.serverRequest)
            this.serverRequest.abort()

        this.setState({ nodeMask: true })
        this.serverRequest = KeePass4Web.fetch('get_entry', {
            method: 'GET',
            data: { id: entry.id },
            success: (data) => {
                this.setState({ entry: null })
                this.setState({ entry: data })
                this.scroll('node-viewer')
            },
            error: KeePass4Web.error.bind(this),
            complete: () => this.setState({ nodeMask: false }),
        })
    }

    onSearch(refs, event) {
        event.preventDefault()

        if (this.serverRequest)
            this.serverRequest.abort()

        this.setState({ entry: null, groupMask: true })

        this.serverRequest = KeePass4Web.fetch('search_entries', {
            method: 'GET',
            data: { term: refs.term.value.replace(/^\s+|\s+$/g, '') },
            success: (data) => {
                this.setState({ group: data, groupMask: false })
                this.scroll('group-viewer')
            },
            error: KeePass4Web.error.bind(this),
            complete: () => this.setState({ groupMask: false }),
        })
    }

    // Re-fetch the current group after a write so the entry list updates
    refreshGroup() {
        if (!this.state.group || !this.state.group.id) return
        KeePass4Web.fetch('get_group_entries', {
            method: 'GET',
            data: { id: this.state.group.id },
            success: (data) => this.setState({ group: data, entry: null }),
            error: KeePass4Web.error.bind(this),
        })
    }

    onEntryCreated() {
        this.refreshGroup()
    }

    onGroupCreated() {
        KeePass4Web.fetch('get_groups', {
            method: 'GET',
            success: (data) => this.setState({ tree: data.groups }),
            error: KeePass4Web.error.bind(this),
        })
    }

    onGroupRenamed(groupId, newTitle) {
        // Patch the title in the tree without a full reload
        const patchTree = (groups) => groups.map(g => ({
            ...g,
            title: g.id === groupId ? newTitle : g.title,
            children: g.children ? patchTree(g.children) : g.children,
        }))
        this.setState(prev => ({
            tree: { ...prev.tree, children: patchTree(prev.tree.children || []) },
            group: prev.group && prev.group.id === groupId
                ? { ...prev.group, title: newTitle }
                : prev.group,
        }))
    }

    onDeleteEntry(entry) {
        if (!window.confirm(`Delete "${entry.title}"?`)) return

        KeePass4Web.fetch('entry', {
            method: 'DELETE',
            data: { id: entry.id },
            success: () => this.refreshGroup(),
            error: KeePass4Web.error.bind(this),
        })
    }

    onSaveDb(e) {
        e.preventDefault()
        this.setState({ saveStatus: 'Saving…' })
        KeePass4Web.fetch('save_db', {
            method: 'POST',
            data: { password: this.state.savePassword },
            success: () => this.setState({ saveModal: false, savePassword: '', saveStatus: '' }),
            error: (err) => {
                this.setState({ saveStatus: err.message || 'Save failed' })
            },
        })
    }

    componentDidMount() {
        KeePass4Web.fetch('get_groups', {
            method: 'GET',
            success: (data) => {
                this.setState({ tree: data.groups })
                if (data.last_selected)
                    this.onGroupSelect({ id: data.last_selected })
            },
            error: KeePass4Web.error.bind(this),
        })
    }

    componentWillUnmount() {
        if (this.serverRequest)
            this.serverRequest.abort()
    }

    render() {
        const saveModal = this.state.saveModal ? (
            <div style={{
                position: 'fixed', top: 0, left: 0, right: 0, bottom: 0,
                background: 'rgba(0,0,0,.5)', zIndex: 1050, display: 'flex',
                alignItems: 'center', justifyContent: 'center',
            }}>
                <div className="panel panel-default" style={{ width: 340, padding: 16 }}>
                    <div className="panel-heading"><b>Save Database to Disk</b></div>
                    <div className="panel-body">
                        <p>Re-enter your master password to write changes to the server file.</p>
                        <form onSubmit={this.onSaveDb}>
                            <div className="form-group">
                                <input
                                    type="password"
                                    className="form-control"
                                    placeholder="Master password"
                                    autoFocus
                                    value={this.state.savePassword}
                                    onChange={e => this.setState({ savePassword: e.target.value, saveStatus: '' })}
                                />
                            </div>
                            {this.state.saveStatus && (
                                <p className="text-danger">{this.state.saveStatus}</p>
                            )}
                            <div className="btn-group">
                                <button type="submit" className="btn btn-primary">Save</button>
                                <button type="button" className="btn btn-default"
                                    onClick={() => this.setState({ saveModal: false, savePassword: '', saveStatus: '' })}>
                                    Cancel
                                </button>
                            </div>
                        </form>
                    </div>
                </div>
            </div>
        ) : null

        return (
            <div className="container-fluid">
                <NavBar
                    showSearch
                    onSearch={this.onSearch}
                    onSaveDb={() => this.setState({ saveModal: true })}
                />
                {saveModal}
                <div className="row">
                    <div className="col-sm-2 dir-tree">
                        <TreeViewer
                            tree={this.state.tree}
                            nodeClick={this.onGroupSelect}
                            nodeIcon="48"
                        />
                    </div>
                    <div id="group-viewer" className="col-sm-4">
                        <GroupViewer
                            group={this.state.group}
                            groupDepth={this.state.groupDepth}
                            onSelect={this.onSelect}
                            mask={this.state.groupMask}
                            onEntryCreated={this.onEntryCreated}
                            onDeleteEntry={this.onDeleteEntry}
                            onGroupRenamed={this.onGroupRenamed}
                            onGroupCreated={this.onGroupCreated}
                        />
                    </div>
                    <div id="node-viewer" className="col-sm-6">
                        <NodeViewer
                            entry={this.state.entry}
                            timeoutSec={30 * 1000}
                            mask={this.state.nodeMask}
                        />
                    </div>
                </div>
            </div>
        )
    }
}

export default withNavigateHook(Viewport)

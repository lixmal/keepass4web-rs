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

        this.state = {
            tree: {},
            entry: null,
            group: null,
        }
    }

    scroll(id) {
        document.getElementById(id).scrollIntoView()
        if (window.scrollY)
            // scroll height of bootstrap fixed header down
            window.scroll(0, scrollY - 70)
    }

    onGroupSelect(group) {
        if (!group || !group.id) return
        if (this.state.group && this.state.group.id && group.id === this.state.group.id) return

        if (this.serverRequest)
            this.serverRequest.abort()

        this.setState({
            entry: null,
            groupMask: true,
        })

        this.serverRequest = KeePass4Web.fetch('get_group_entries', {
            method: "GET",
            data: {
                id: group.id,
            },
            success: function (data) {
                this.setState({
                    group: data,
                })

                this.scroll('group-viewer')
            }.bind(this),
            error: KeePass4Web.error.bind(this),
            complete: function () {
                this.setState({
                    groupMask: false,
                })
            }.bind(this)
        })
    }

    onSelect(entry) {
        if (!entry || !entry.id) return
        // ignore already selected
        if (this.state.entry && this.state.entry.id && entry.id === this.state.entry.id) return

        if (this.serverRequest)
            this.serverRequest.abort()

        this.setState({
            nodeMask: true,
        })
        this.serverRequest = KeePass4Web.fetch('get_entry', {
            method: "GET",
            data: {
                id: entry.id,
            },
            success: function (data) {
                // remove entry first to rerender entry
                // important for eye close/open buttons
                this.setState({
                    entry: null,
                })
                this.setState({
                    entry: data,
                })

                this.scroll('node-viewer')
            }.bind(this),
            error: KeePass4Web.error.bind(this),
            complete: function () {
                this.setState({
                    nodeMask: false,
                })
            }.bind(this)
        })
    }

    onSearch(refs, event) {
        event.preventDefault()

        if (this.serverRequest)
            this.serverRequest.abort()

        this.setState({
            entry: null,
            groupMask: true,
        })

        this.serverRequest = KeePass4Web.fetch('search_entries', {
            method: "GET",
            data: {
                term: refs.term.value.replace(/^\s+|\s+$/g, ''),
            },
            success: function (data) {
                this.setState({
                    group: data,
                    groupMask: false,
                })

                this.scroll('group-viewer')
            }.bind(this),
            error: KeePass4Web.error.bind(this),
            complete: function () {
                this.setState({
                    groupMask: false,
                })
            }.bind(this)
        })
    }

    componentDidMount() {
        // TODO: add loading mask for the whole viewport while fetching groups
        KeePass4Web.fetch('get_groups', {
            method: "GET",
            success: function (data) {
                this.setState({
                    tree: data.groups
                })
                if (data.last_selected)
                    this.onGroupSelect({
                        id: data.last_selected
                    })
            }.bind(this),
            error: KeePass4Web.error.bind(this),
        })
    }

    componentWillUnmount() {
        if (this.serverRequest)
            this.serverRequest.abort()
    }

    render() {
        return (
            <div className="container-fluid">
                <NavBar
                    showSearch
                    onSearch={this.onSearch}
                />
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
                            onSelect={this.onSelect}
                            mask={this.state.groupMask}
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

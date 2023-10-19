import React from 'react'
import withNavigateHook from './nagivateHook'


class TreeNode extends React.Component {
    constructor(props) {
        super(props)

        let node = props.node
        this.state = {
            expanded: node.hasOwnProperty('expanded') ?
                node.expanded :
                props.level < (props.options.levels || 3) ? true : false
        }
    }

    toggleExpanded(event) {
        this.setState({expanded: !this.state.expanded})
        event.stopPropagation()
    }

    select(node, event) {
        let nodeClick = this.props.options.nodeClick
        if (nodeClick)
            nodeClick(node)
        event.stopPropagation()
    }

    render() {
        let node = this.props.node
        let options = this.props.options
        let showBorder = typeof options.showBorder === 'undefined' ? true : options.showBorder

        let style
        if (!this.props.visible) {
            style = {
                display: 'none'
            }
        } else {
            if (!showBorder) {
                style.border = 'none'
            } else if (options.borderColor) {
                style.border = '1px solid ' + options.borderColor
            }
        }

        let indents = []
        for (let i = 0; i < this.props.level - 1; i++) {
            indents.push(<span className="indent" key={i}></span>)
        }

        let expandCollapseIcon
        if (node.children) {
            if (!this.state.expanded) {
                expandCollapseIcon = (
                    <span className={options.expandIcon || 'glyphicon glyphicon-plus'}
                          onClick={this.toggleExpanded.bind(this)}>
                    </span>
                )
            } else {
                expandCollapseIcon = (
                    <span className={options.collapseIcon || 'glyphicon glyphicon-minus'}
                          onClick={this.toggleExpanded.bind(this)}>
                    </span>
                )
            }
        } else {
            expandCollapseIcon = (
                <span className={options.emptyIcon || 'glyphicon glyphicon-none'}></span>
            )
        }

        let srcurl
        if (node.custom_icon_uuid) {
            srcurl = 'icon/' + encodeURIComponent(node.custom_icon_uuid)
        } else {
            srcurl = 'assets/img/icons/' + encodeURIComponent(node.icon || options.nodeIcon || '48') + '.png'
        }
        let nodeIcon = (
            <img src={srcurl} className="kp-icon icon"/>
        )

        let children = []
        if (node.children) {
            let nodes = node.children
            for (let i in nodes) {
                children.push(
                    <TreeNode
                        node={nodes[i]}
                        level={this.props.level + 1}
                        visible={this.state.expanded && this.props.visible}
                        options={options}
                        key={nodes[i].id}
                    />
                )
            }
        }

        return (
            <div>
                <li className="list-group-item"
                    style={style}
                    onClick={this.select.bind(this, node)}
                >
                    {indents}
                    {expandCollapseIcon}
                    {nodeIcon}
                    {node.title}
                </li>
                {children}
            </div>
        )
    }
}

export default withNavigateHook(TreeNode)

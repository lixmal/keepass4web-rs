import React from 'react'
import Classnames from 'classnames'

import withNavigateHook from './nagivateHook'


class GroupViewer extends React.Component {
    constructor(props) {
        super(props)
    }

    getIcon(element) {
        if (element.custom_icon_uuid)
            return <img className="kp-icon" src={'icon/' + encodeURIComponent(element.custom_icon_uuid)}/>
        else if (element.icon)
            return <img className="kp-icon" src={'assets/img/icons/' + encodeURIComponent(element.icon) + '.png'}/>
    }

    render() {
        const classes = Classnames({
            'panel': true,
            'panel-default': true,
            'loading-mask': this.props.mask,
        })

        if (!this.props.group) return (<div className={classes}></div>)

        const group = this.props.group

        let entries = []
        for (var i in group.entries) {
            let entry = group.entries[i]

            entries.push(
                <tr key={i} onClick={this.props.onSelect.bind(this, entry)}>
                    <td className="kp-wrap">
                        {this.getIcon(entry)}
                        {entry.title}
                    </td>
                    <td className="kp-wrap">
                        {entry.username}
                    </td>
                </tr>
            )
        }

        return (
            <div className={classes}>
                <div className="panel-heading">
                    {this.getIcon(group)}
                    {group.title}
                </div>
                <div className="panel-body">
                    <table className="table table-hover table-condensed kp-table">
                        <thead>
                        <tr>
                            <th>
                                Entry Name
                            </th>
                            <th>
                                Username
                            </th>
                        </tr>
                        </thead>
                        <tbody className="groupview-body">
                        {entries}
                        </tbody>
                    </table>
                </div>
            </div>
        )
    }
}

export default withNavigateHook(GroupViewer)

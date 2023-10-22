import React from 'react'


import NavBar from "./NavBar"
import withNavigateHook from "./nagivateHook"

class Splash extends React.Component {

    componentDidMount() {
        // TODO: cancel this on unmount
        KeePass4Web.checkAuth.call(this, {}, function () {
            this.props.navigate('/keepass', {replace: true})
        }.bind(this))
    }

    render() {
        // TODO: show errors here
        return (
            <div className={"loading-mask"}>
                <NavBar/>
            </div>
        )
    }
}

export default withNavigateHook(Splash)
